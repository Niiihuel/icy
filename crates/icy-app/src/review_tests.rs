use super::*;
use jsnir::host::{Frame, Item};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

fn frame(host: &mut Host) -> Frame {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        host.poll();
        let frame = host.frame(640., 800., 1., 4096);
        assert!(host.dev_error().is_none(), "{:?}", host.dev_error());
        if (!host.busy() && !frame.animating) || Instant::now() >= deadline {
            return frame;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn workspaces_button_draws_the_grid_glyph() {
    let mut host = Host::with_request_handler(
        format!("{APP}\nmount(TabBar);"),
        "{}".into(),
        TextEngine::default(),
        |_| Ok("ok".into()),
    )
    .unwrap();
    let grid = icons::load_static(1.0)
        .into_iter()
        .find(|(name, _, _)| name == "glyph:workspaces")
        .unwrap();
    host.set_image(grid.0, grid.1.clone(), grid.2);
    assert!(frame(&mut host).items.iter().any(|item| matches!(item,
        Item::Image { image, .. } if image.rgba.as_ref() == grid.1.as_slice())));
}

#[test]
fn release_notes_glow_follows_hover_and_recolors_when_the_theme_changes() {
    let script = format!(
        r#"{APP}
        viewportSize.value = [640, 800];
        const testMenu = signal(true);
        let testAnchor;
        mount(() => Column({{children: [
          Box({{style:{{width: 240, height: 32}}, ref: id => {{testAnchor = id;}}}}),
          AccountMenu({{open: testMenu, anchor: () => testAnchor, side: 'bottom'}}),
          ReleaseNotesModal(),
        ]}}));
    "#
    );
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    host.set_reduced_motion(true);
    install_fonts(&mut host);
    install_images_on_host(&mut host, 1.0);
    host.push(json!({"view": "update", "data": updater::UpdateState::initial()}).to_string());
    let first = frame(&mut host);
    let glow = |frame: &Frame| {
        frame.items.iter().find_map(|item| match item {
            Item::Image { image, rect, .. } if image.size == [384, 44] => {
                Some((*rect, image.rgba.as_ref().to_vec()))
            }
            _ => None,
        })
    };
    assert!(glow(&first).is_none());
    let row = first
        .items
        .iter()
        .find_map(|item| match item {
            Item::Text { text, rect, .. } if text == "Release notes" => Some(*rect),
            _ => None,
        })
        .unwrap();
    let pointer = [row[0] + 8.0, row[1] + row[3] / 2.0];
    host.pointer_move(Some(pointer));
    frame(&mut host);
    let target = rx
        .try_iter()
        .find(|request| request["type"] == "releaseGlowTarget" && request["inside"] == true)
        .expect("releaseGlowTarget request");
    let blue_colors = target["colors"].clone();
    release_glow::request(&target).unwrap();
    let mut sampler = release_glow::Sampler::default();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !sampler.sample(&mut host, Some(pointer)) || glow(&frame(&mut host)).is_none() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for release:tab-gradient"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let (initial, _) = glow(&frame(&mut host)).expect("visible hover glow");
    // Follow starts at 154.5 and eases left toward the pointer near the row's start.
    std::thread::sleep(Duration::from_millis(80));
    assert!(sampler.sample(&mut host, Some(pointer)));
    assert!(glow(&frame(&mut host)).unwrap().0[0] < initial[0]);
    host.push(json!({"view":"config","data":{"theme":"accordee-light","mode":"light","accent":"rose","fontSize":13}}).to_string());
    frame(&mut host);
    let rose_target = rx
        .try_iter()
        .filter(|request| request["type"] == "releaseGlowTarget" && request["inside"] == true)
        .last()
        .expect("recolored releaseGlowTarget");
    assert_ne!(rose_target["colors"], blue_colors);
    host.pointer_down(jsnir::editor::PointSelection::Place);
    host.pointer_up();
    let modal = frame(&mut host);
    assert!(modal.items.iter().any(|item| matches!(item,
        Item::Text { text, .. } if text.starts_with("icy v"))));
    assert!(modal.items.iter().any(|item| matches!(item,
        Item::Text { text, .. } if text == "No release notes published for this version yet.")));
    host.escape();
    let closed = frame(&mut host);
    assert!(!closed.items.iter().any(|item| matches!(item,
        Item::Text { text, .. } if text.starts_with("icy v"))));
    assert!(
        rx.try_iter()
            .any(|request| request["type"] == "releaseGlowTarget" && request["inside"] == false)
    );
}

#[test]
fn release_notes_cover_switches_between_ascii_image_and_inline_video() {
    use jsnir::editor::PointSelection;

    const POSTER: &str = "https://example.com/workspace.png";
    const VIDEO: &str = "https://example.com/demo.webm";
    const PICTURE: [u8; 4] = [17, 32, 49, 255];
    const PLAY: [u8; 4] = [75, 85, 95, 255];

    let script = format!(
        r#"{APP}
        viewportSize.value = [640, 800];
        releaseNotesOpen.value = true;
        mount(ReleaseNotesModal);
    "#
    );
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    host.set_reduced_motion(true);
    install_fonts(&mut host);
    host.set_image("test:cover".into(), PICTURE.to_vec(), [1, 1]);
    host.set_image("glyph:play".into(), PLAY.to_vec(), [1, 1]);

    let has_ascii = |frame: &Frame| {
        frame.items.iter().any(|item| {
            matches!(item,
        Item::Text { text, .. } if text.contains("*----+----+"))
        })
    };
    let picture = |frame: &Frame, marker: [u8; 4]| {
        frame.items.iter().find_map(|item| match item {
            Item::Image {
                image, rect, clip, ..
            } if image.rgba.as_ref() == marker => Some((*rect, *clip)),
            _ => None,
        })
    };
    assert!(has_ascii(&frame(&mut host)));

    let mut state = updater::UpdateState::initial();
    state.notes.media.push(updater::ReleaseMedia {
        kind: "image".into(),
        title: "Workspace".into(),
        url: POSTER.into(),
        duration: String::new(),
        caption: String::new(),
        poster_url: None,
    });
    host.push(json!({"view": "update", "data": &state}).to_string());
    frame(&mut host);
    assert!(
        rx.try_iter()
            .any(|request| request == json!({"type": "releaseCover", "url": POSTER}))
    );
    host.push(
        json!({"view": "fileImage", "data": {
            "path": POSTER, "key": "test:cover", "width": 1, "height": 1,
        }})
        .to_string(),
    );
    let still = frame(&mut host);
    let (cover, clip) = picture(&still, PICTURE).expect("image cover");
    assert_eq!(cover[2], 520.);
    assert!(cover[3] > clip[3], "a square source is center-cropped");
    assert!(!has_ascii(&still));

    let media = &mut state.notes.media[0];
    media.kind = "video".into();
    media.url = VIDEO.into();
    media.poster_url = Some(POSTER.into());
    host.push(json!({"view": "update", "data": &state}).to_string());
    let video = frame(&mut host);
    assert!(picture(&video, PICTURE).is_some());
    let (play, _) = picture(&video, PLAY).expect("video playback control");
    host.pointer_move(Some([play[0] + play[2] / 2., play[1] + play[3] / 2.]));
    host.pointer_down(PointSelection::Place);
    host.pointer_up();
    frame(&mut host);
    let player = rx
        .try_iter()
        .find(|request| request["type"] == "browser" && request["op"] == "open")
        .expect("inline video opens a browser surface");
    assert_eq!(player["width"], 520);
    assert!(player["url"].as_str().unwrap().contains("demo.webm"));
    host.push(
        json!({"view": "browserState", "data": {
            "id": player["id"], "url": VIDEO, "title": "", "loading": false,
            "canBack": false, "canForward": false, "error": "CEF unavailable",
        }})
        .to_string(),
    );
    let fallback = frame(&mut host);
    let open_video = fallback
        .items
        .iter()
        .find_map(|item| match item {
            Item::Text { text, rect, .. } if text == "Open video" => Some(*rect),
            _ => None,
        })
        .expect("fallback open-video button");
    host.pointer_move(Some([
        open_video[0] + open_video[2] / 2.,
        open_video[1] + open_video[3] / 2.,
    ]));
    host.pointer_down(PointSelection::Place);
    host.pointer_up();
    frame(&mut host);
    assert!(
        rx.try_iter()
            .any(|request| request == json!({"type": "openExternal", "url": VIDEO}))
    );
    host.escape();
    frame(&mut host);
    assert!(rx.try_iter().any(|request| request
        == json!({
            "type": "browser", "op": "close", "id": player["id"],
        })));
}

#[test]
fn terminal_right_edge_stays_in_one_column_and_pty_fits_its_viewport() {
    use icy_core::vt::{AlacrittyVt, VtEngine};
    let mut vt = AlacrittyVt::new(77, 6);
    vt.feed(
        "\x1b[48;2;17;34;51m┌───┐\x1b[77G\x1b[31m│\r\n\x1b[39m☆雪e\u{301}\x1b[77G\x1b[31m│"
            .as_bytes(),
    );
    let rows = terminal_rows_payload(&vt.snapshot().rows, Path::new("."));
    let data = json!({"cols":77,"rowsCount":6,"cursorRow":1,"cursorCol":76,"cursorVisible":true,"rows":rows});
    let (tx, rx) = mpsc::channel();
    let mut host = Host::with_request_handler(
        format!(r#"{APP}
            screensMapState.value = {{'1': {data}}};
            sessionsState.value = {{activeId: 1, items: [{{id:1, title:'OpenCode', cli:'opencode', cwd:'/project', alive:true, status:'working'}}]}};
            mount(() => TerminalPane({{sessionId: () => 1, focused: () => true, multiPane: () => false}}));"#),
        "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap()).unwrap();
            Ok("ok".into())
        },
    ).unwrap();
    install_fonts(&mut host);
    let initial = frame(&mut host);
    let rule_positions: Vec<_> = initial
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Text {
                text, rect, clip, ..
            } if text == "│" => Some((*rect, *clip)),
            _ => None,
        })
        .collect();
    assert_eq!(
        rule_positions.len(),
        2,
        "the rightmost cells must be visible"
    );
    assert_eq!(rule_positions[0].0[0], rule_positions[1].0[0]);
    assert!((rule_positions[0].0[0] - (16. + 76. * 7.8)).abs() <= 1.);
    assert_eq!(
        rule_positions[0].1, rule_positions[1].1,
        "both rows share the grid clip"
    );
    assert!(rule_positions[0].1[2] > 600., "the clip covers the grid");
    assert!(initial.items.iter().any(|item| matches!(item,
        Item::Text { text, spans: None, .. } if text == "┌───┐")));
    std::thread::sleep(Duration::from_millis(60));
    frame(&mut host);
    let size = rx
        .try_iter()
        .find(|view| view["type"] == "terminalSize")
        .unwrap();
    assert_eq!(size["cols"], 77);
    assert_eq!(size["rows"], 42);
}

#[test]
fn sessions_render_reported_activity_even_when_the_cli_title_does_not_change() {
    let mut host = Host::with_request_handler(
        format!(r#"{APP}
            sessionsState.value = {{activeId: 1, items: [{{id:1,title:'OpenCode',cwd:'/project',cli:'opencode',alive:true,status:'idle'}}]}};
            mount(PriorityAgents);"#),
        "{}".into(), TextEngine::default(), |_| Ok("ok".into()),
    ).unwrap();
    for (status, label) in [
        ("idle", "Idle"),
        ("working", "Working"),
        ("blocked", "Needs you"),
        ("done", "Done"),
    ] {
        host.push(json!({ "view": "sessions", "data": { "activeId": 1, "items": [
            {"id":1,"title":"OpenCode","cwd":"/project","cli":"opencode","alive":true,"status":status}
        ] } }).to_string());
        let frame = frame(&mut host);
        assert!(
            frame
                .items
                .iter()
                .any(|item| matches!(item, Item::Text { text, .. } if text == label)),
            "{status}"
        );
    }
}

#[test]
fn alt_agent_digits_follow_the_visible_sort_and_filter() {
    let script = format!(
        r#"{APP}
        mount(PriorityAgents);
        const originalHandler = hostHandler;
        onHost(m => {{ originalHandler?.(m); if (m.view === 'testSort') {{agentSort.value = 'project'; sessionFilter.value = 'working';}} }});
    "#
    );
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    host.push(
        json!({"view":"sessions","data":{"activeId":2,"items":[
            {"id":2,"title":"Waiting","cli":"codex","alive":true,"cwd":"/test","status":"idle"},
            {"id":3,"title":"Busy","cli":"opencode","alive":true,"cwd":"/test","status":"working"},
            {"id":4,"title":"Approval","cli":"claude","alive":true,"cwd":"/test","status":"blocked"}
        ]}})
        .to_string(),
    );
    host.push(json!({"view":"agentDigitsHeld","data":true}).to_string());
    let active = frame(&mut host);
    let text_labels: Vec<_> = active
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(text_labels.contains(&"3"), "{text_labels:?}");
    host.push(json!({"view":"agentDigit","data":1}).to_string());
    frame(&mut host);
    assert!(
        rx.try_iter()
            .any(|request| request["type"] == "selectSession" && request["id"] == 4)
    );
    host.push(json!({"view":"testSort"}).to_string());
    host.push(json!({"view":"agentDigit","data":1}).to_string());
    frame(&mut host);
    assert!(
        rx.try_iter()
            .any(|request| request["type"] == "selectSession" && request["id"] == 3)
    );
    host.push(json!({"view":"agentDigitsHeld","data":false}).to_string());
    assert!(
        !frame(&mut host)
            .items
            .iter()
            .any(|item| matches!(item, Item::Text { text, .. } if text == "3"))
    );
}

#[test]
fn font_file_uses_jsnir_without_joining_the_ui_font_database() {
    let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let view = open_file_for_viewer("assets/fonts/GeistMono-Regular.ttf", None, None, &cwd);
    assert!(view.error.is_none());
    let Some(FilePreview::Font { name, size, key }) = &view.preview else {
        panic!("expected a font sample");
    };
    assert!(name.contains("Geist"), "{name}");
    let pixels = view.preview_pixels.as_ref().unwrap();
    assert_eq!(pixels.len(), (size[0] * size[1] * 4) as usize);
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] > 0));
    assert!(view.editable_text.is_none());
    let mut host = Host::with_request_handler(
        format!("{APP}\nconst sample = {};\nmount(() => SpecialFilePreview({{keyForDoc: () => sample}}));", serde_json::to_string(&view).unwrap()),
        "{}".into(), TextEngine::default(), |_| Ok("ok".into()),
    ).unwrap();
    let epoch = host.font_epoch();
    host.set_image(key.clone(), pixels.clone(), *size);
    let frame = frame(&mut host);
    assert!(
        frame
            .items
            .iter()
            .any(|item| matches!(item, Item::Image { image, .. } if image.size == *size))
    );
    assert_eq!(host.font_epoch(), epoch);
}

#[test]
fn source_editor_drag_selections_and_native_edits() {
    let script = format!(
        r#"{APP}
        const spec = {{ kind: 'file', path: 'test.txt', cwd: '/project' }};
        const fixture = {{ path: '/project/test.txt', displayPath: 'test.txt', editableText: 'one\ntwo 🌊\nthree\n',
          lines: ['one', 'two 🌊', 'three'].map((text, i) => ({{ num: i + 1, text }})) }};
        sessionsState.value = {{ items: [
          {{ id: 11, title: 'Codex session', cli: 'codex', cwd: '/project', alive: true }},
          {{ id: 42, title: 'Claude session', cli: 'claude', cwd: '/project', alive: true }},
          {{ id: 44, title: 'Other project', cli: 'codex', cwd: '/other', alive: true }},
          {{ id: 45, title: 'Plain shell', cli: 'terminal', cwd: '/project', alive: true }}
        ] }};
        mount(() => SourceEditor({{ spec, doc: () => fixture }}));
        onHost(() => saveEditorDraft(editorDrafts.get(docKey(spec))));
        if (editorSelection('test.txt', '🌊ab\nc', [5, 1]) !== 'test.txt:1-2\n\nab\nc') throw new Error('wrong Unicode selection');
        "#
    );
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    let text_point = |frame: &Frame, needle: &str| -> [f32; 2] {
        frame
            .items
            .iter()
            .find_map(|item| match item {
                Item::Text { text, rect, .. } if text == needle => {
                    Some([rect[0] + 4., rect[1] + rect[3] / 2.])
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing text: {needle}"))
    };
    // Start just left of the first line's text and end past the last line's end.
    let edge = |frame: &Frame, needle: &str, right: bool| -> [f32; 2] {
        let rect = frame
            .items
            .iter()
            .find_map(|item| match item {
                Item::Text { text, rect, .. } if text == needle => Some(*rect),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing text: {needle}"));
        [
            if right {
                rect[0] + rect[2] - 2.
            } else {
                rect[0] - 2.
            },
            rect[1] + rect[3] / 2.,
        ]
    };
    let initial = frame(&mut host);
    host.pointer_move(Some(edge(&initial, "one", false)));
    host.pointer_down(jsnir::editor::PointSelection::Place);
    host.pointer_move(Some(edge(&initial, "three", true)));
    host.pointer_up();
    let selected = frame(&mut host);
    text_point(&selected, "Lines 1–3");
    host.pointer_move(Some(text_point(&selected, "Send to CLI…")));
    host.pointer_down(jsnir::editor::PointSelection::Place);
    host.pointer_up();
    let menu = frame(&mut host);
    assert!(!menu.items.iter().any(|item| matches!(item, Item::Text { text, .. } if text == "Plain shell" || text == "Other project")));
    host.pointer_move(Some(text_point(&menu, "Claude session")));
    host.pointer_down(jsnir::editor::PointSelection::Place);
    host.pointer_up();
    let selected = frame(&mut host);
    let paste = rx
        .try_iter()
        .find(|v| v["type"] == "pasteEditorContext")
        .unwrap();
    assert_eq!(paste["id"], 42);
    assert_eq!(paste["text"], "test.txt:1-3\n\none\ntwo 🌊\nthree");
    host.pointer_move(Some(text_point(&selected, "Edit")));
    host.pointer_down(jsnir::editor::PointSelection::Place);
    host.pointer_up();
    frame(&mut host);
    assert!(host.focused());
    host.edit(&[jsnir::editor::EditorEvent::Insert("extra".into())]);
    frame(&mut host);
    host.push("{}".into());
    frame(&mut host);
    let request = rx.try_iter().find(|v| v["type"] == "saveEditor").unwrap();
    assert_eq!(request["expected"], "one\ntwo 🌊\nthree\n");
    assert_eq!(request["text"], "one\ntwo 🌊\nthree\nextra");
}

#[test]
fn source_editor_char_selection_chip_footer_and_blame() {
    let script = format!(
        r#"{APP}
        const spec = {{ kind: 'file', path: 'test.txt', cwd: '/project' }};
        const fixture = {{ path: '/project/test.txt', displayPath: 'test.txt', editableText: 'alpha beta\n  gamma\n',
          textInfo: {{ language: 'Plain Text', bom: false, crlf: true, indent: {{ kind: 'spaces', size: 2 }} }},
          lines: ['alpha beta', '  gamma'].map((text, i) => ({{ num: i + 1, text }})) }};
        sessionsState.value = {{ items: [
          {{ id: 11, title: 'Codex session', cli: 'codex', cwd: '/project', alive: true }},
          {{ id: 42, title: 'Claude session', cli: 'claude', cwd: '/project', alive: true }}
        ] }};
        activeNav.value = 'agents';
        tileSpecs.value = {{ 900: spec }};
        tilingState.value = {{ ...tilingState.value, focusedSession: 900 }};
        mount(() => SourceEditor({{ spec, doc: () => fixture }}));
        "#
    );
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    let has_text = |frame: &Frame, needle: &str| {
        frame
            .items
            .iter()
            .any(|item| matches!(item, Item::Text { text, .. } if text == needle))
    };
    let rect_of = |frame: &Frame, needle: &str| -> [f32; 4] {
        frame
            .items
            .iter()
            .rev()
            .find_map(|item| match item {
                Item::Text { text, rect, .. } if text == needle => Some(*rect),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing text: {needle}"))
    };
    let key = |host: &mut Host, action: &str| {
        host.push(json!({"view": "modalKey", "data": action}).to_string());
        frame(host)
    };
    // The tiling snapshot the page receives when a pane takes focus.
    let focus = |host: &mut Host, id: u64| {
        host.push(json!({"view": "tiling", "data": {
            "mode": "niri", "focusedSession": id, "maximized": false, "viewOffset": 0, "stripWidth": 0, "totalCols": 1,
            "columns": [{"id": 1, "panes": [id], "heights": [1], "x": 0, "width": 1, "expanded": false, "visible": true}],
            "workspaces": [{"panes": [id], "focused": id, "columns": [], "viewOffset": 0}],
            "activeWorkspace": 0
        }}).to_string());
        frame(host)
    };
    let mut requests: Vec<Value> = Vec::new();

    let initial = frame(&mut host);
    for text in ["UTF-8", "CRLF", "Spaces: 2", "Plain Text"] {
        assert!(has_text(&initial, text), "missing footer item {text}");
    }
    assert!(
        !initial
            .items
            .iter()
            .any(|item| matches!(item, Item::Text { text, .. } if text.starts_with("Ln ")))
    );

    let alpha = rect_of(&initial, "alpha beta");
    let gamma = rect_of(&initial, "  gamma");
    host.pointer_move(Some([alpha[0] + 6. * 7.5 + 1., alpha[1] + alpha[3] / 2.]));
    host.pointer_down(jsnir::editor::PointSelection::Place);
    host.pointer_move(Some([gamma[0] + 4. * 7.5 + 1., gamma[1] + gamma[3] / 2.]));
    host.pointer_up();
    let selected = frame(&mut host);
    assert!(has_text(&selected, "Ln 2, Col 5 (9 selected)"));
    assert!(has_text(&selected, "Add to Chat"));

    // Two CLIs and no recently focused one: the session menu opens instead of pasting.
    let menu = key(&mut host, "editor.addToChat");
    requests.extend(rx.try_iter());
    assert!(has_text(&menu, "Codex session"));
    assert!(requests.iter().all(|v| v["type"] != "pasteEditorContext"));

    // Codex was focused most recently, so it becomes the target.
    focus(&mut host, 11);
    focus(&mut host, 900);
    key(&mut host, "editor.addToChat");
    requests.extend(rx.try_iter());
    let paste = requests
        .iter()
        .find(|v| v["type"] == "pasteEditorContext")
        .expect("paste request");
    assert_eq!(paste["id"], 11);
    assert_eq!(paste["text"], "test.txt:1-2\n\nbeta\n  ga");

    let blame_asked = |requests: &[Value]| {
        requests
            .iter()
            .any(|v| v["type"] == "gitBlame" && v["line"] == 2)
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    while !blame_asked(&requests) && Instant::now() < deadline {
        frame(&mut host);
        requests.extend(rx.try_iter());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(blame_asked(&requests), "no blame request: {requests:?}");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    host.push(json!({"view": "gitBlame", "data": {
        "path": "/project/test.txt", "line": 2, "branch": "main",
        "blame": {"author": "Ada", "time": now - 2 * 86400 - 60, "summary": "x", "uncommitted": false}
    }}).to_string());
    let blamed = frame(&mut host);
    assert!(has_text(&blamed, "main"));
    assert!(has_text(&blamed, "Ada, 2d ago"));
}

#[test]
fn viewer_reports_text_info_for_the_footer() {
    let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let view = open_file_for_viewer("Cargo.toml", None, None, &cwd);
    assert_eq!(view.text_info.unwrap().language, "TOML");
}

#[test]
fn lazy_rows_do_not_rebuild_the_source_or_height_index_on_scroll() {
    let script = format!(
        r#"{APP}
        let builds = 0, estimates = 0, mounted = 0;
        const rows = Array.from({{length: 10000}}, (_, i) => ({{key: String(i)}}));
        mount(() => LazyRows({{
            items: () => {{ builds++; return rows; }},
            estimate: () => {{ estimates++; return 20; }},
            key: row => row.key,
            children: item => {{
                mounted++;
                onCleanup(() => mounted--);
                return Text({{style: {{height: 20, shrink: 0}}, children: () => item().key}});
            }}
        }}));
        onHost(() => requestHost({{type: 'stats', builds, estimates, mounted}}));
        "#
    );
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    frame(&mut host);
    host.push("{}".into());
    frame(&mut host);
    let before = rx.try_iter().last().expect("initial stats");
    assert_eq!(before["builds"], 1);
    assert!(before["mounted"].as_u64().unwrap() < 120);

    host.pointer_move(Some([320., 400.]));
    for _ in 0..8 {
        assert!(host.wheel(250.));
        frame(&mut host);
    }
    host.push("{}".into());
    frame(&mut host);
    let after = rx.try_iter().last().expect("scroll stats");
    assert_eq!(after["builds"], before["builds"]);
    assert_eq!(after["estimates"], before["estimates"]);
    assert!(after["mounted"].as_u64().unwrap() < 120);
}

#[test]
fn review_renders_changes_at_both_ends_of_a_large_list() {
    let script = format!("{APP}\nmount(() => ReviewView({{cwd: '/project'}}));");
    let mut host = Host::with_request_handler(script, "{}".into(), TextEngine::default(), |_| {
        Ok("ok".into())
    })
    .unwrap();
    install_fonts(&mut host);
    let files: Vec<_> = (0..1000)
        .map(|i| json!({"path": format!("src/file-{i}.rs"), "status": "M", "additions": 1, "deletions": 0}))
        .collect();
    host.push(
        json!({"view": "changes", "data": {"cwd": "/project", "changes": {
            "files": files, "additions": 1000, "deletions": 0
        }}})
        .to_string(),
    );
    let has_text = |frame: &Frame, value: &str| {
        frame
            .items
            .iter()
            .any(|item| matches!(item, Item::Text { text, .. } if text == value))
    };
    assert!(has_text(&frame(&mut host), "file-0.rs"));
    host.pointer_move(Some([320., 400.]));
    assert!(host.wheel(100_000.));
    assert!(has_text(&frame(&mut host), "file-999.rs"));
    assert!(host.wheel(-100_000.));
    assert!(has_text(&frame(&mut host), "file-0.rs"));
}

#[test]
fn changes_reuse_unchanged_items_and_coalesce_inflight_refreshes() {
    let script = format!(
        r#"{APP}
        const assert = (ok, message) => {{ if (!ok) throw new Error(message); }};
        const changes = {{files: [
            {{path: 'a.rs', status: 'M', additions: 1, deletions: 0}},
            {{path: 'b.rs', status: 'M', additions: 2, deletions: 0}}
        ], additions: 3, deletions: 0}};
        receiveChanges('/project', changes);
        const first = projectChanges('/project');
        receiveChanges('/project', JSON.parse(JSON.stringify(changes)));
        assert(projectChanges('/project') === first, 'no-op refresh changed the source');
        const next = JSON.parse(JSON.stringify(changes));
        next.files[1].additions = 3;
        next.additions = 4;
        receiveChanges('/project', next);
        const second = projectChanges('/project');
        assert(second.files[0] === first.files[0], 'unchanged row was replaced');
        assert(second.files[1] !== first.files[1], 'changed row was not replaced');
        requestChanges('/project');
        requestChanges('/project');
        receiveChanges('/project', next);
        requestChanges('/project');
        mount(() => Text({{children: 'passed'}}));
        "#
    );
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    frame(&mut host);
    assert_eq!(
        rx.try_iter().filter(|v| v["type"] == "gitChanges").count(),
        2
    );
}

#[test]
fn document_loader_returns_background_results_without_ui_work() {
    let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let (tx, rx) = mpsc::channel();
    spawn_viewer_probe(
        ViewerRequest {
            path: "Cargo.toml".into(),
            cwd: cwd.clone(),
            diff: false,
            line: Some(1),
            col: None,
            old_line: false,
            edit: None,
            background: true,
        },
        tx,
    )
    .unwrap();
    let BridgeEvent::PushViewer(view) = rx.recv_timeout(Duration::from_secs(5)).unwrap() else {
        panic!("expected document result");
    };
    assert!(view.error.is_none());
    assert!(view.background);
    assert_eq!(view.cwd, cwd);
    assert_eq!(view.requested_path, "Cargo.toml");
    assert!(view.lines[0].highlighted);
}

#[test]
fn terminal_edit_link_opens_the_changed_hunk_beyond_twenty_thousand_lines() {
    use icy_core::vt::{AlacrittyVt, VtEngine};

    let cwd = std::env::temp_dir().join(format!("icy-cli-diff-{}", std::process::id()));
    std::fs::create_dir_all(&cwd).unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&cwd)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init"]);
    let path = cwd.join("sample.rs");
    let mut lines: Vec<_> = (1..=21_000).map(|i| format!("// line {i}\n")).collect();
    std::fs::write(&path, lines.concat()).unwrap();
    git(&["add", "sample.rs"]);
    git(&[
        "-c",
        "user.name=icy test",
        "-c",
        "user.email=icy@example.test",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-m",
        "Base",
    ]);
    for i in (100..20_000).step_by(50) {
        lines[i] = format!("// changed {i}\n");
    }
    lines[20_898] = "// a neighboring edit\n".into();
    lines[20_899] = "fn edited_again_later() {}\n".into();
    std::fs::write(path, lines.concat()).unwrap();

    let mut vt = AlacrittyVt::new(34, 10);
    vt.feed("└ Updated sample.rs +1 -1\r\n  20899   // line 20899\r\n  20900 -// line 20900\r\n  20900 +fn edited_in_terminal() {}\r\nSee sample.rs:20899".as_bytes());
    let rows = terminal_rows_payload(&vt.snapshot().rows, &cwd);
    assert_eq!(rows[0].links[0].kind, RefKind::Diff);
    assert_eq!(rows[0].links[0].line, Some(20_900));
    assert_eq!(rows[5].links[0].kind, RefKind::File);

    let manager = Arc::new(Mutex::new(SessionManager::new(
        cwd.clone(),
        Arc::new(|| {}),
    )));
    let (tx, rx) = mpsc::channel();
    let script = format!(
        r#"{APP}
        const links = [{}, {}];
        const appHostHandler = hostHandler;
        let nextLink = 0;
        onHost(message => {{
            if (message.clickLink) openLink(links[nextLink++]);
            else appHostHandler?.(message);
        }});
        mount(() => DocView({{spec: {{
            kind: "diff", path: "sample.rs", cwd: {}, edit: links[0].edit,
            line: links[0].line, oldLine: links[0].oldLine,
        }}}}));
        "#,
        serde_json::to_string(&rows[0].links[0]).unwrap(),
        serde_json::to_string(&rows[5].links[0]).unwrap(),
        serde_json::to_string(&cwd).unwrap(),
    );
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            handle_request(&raw, &manager, &tx)
        })
        .unwrap();
    install_fonts(&mut host);

    host.push(r#"{"clickLink":true}"#.into());
    frame(&mut host);
    let next_view = || loop {
        if let BridgeEvent::PushViewer(view) = rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            break view;
        }
    };
    let mut diff = next_view();
    assert_eq!(diff.mode, "diff");
    assert!(diff.error.is_none(), "{:?}", diff.error);
    assert!(diff.lines.is_empty());
    assert_eq!(
        diff.diff[diff.diff_focus.unwrap()].line.old_num,
        Some(20_900)
    );
    assert_eq!((diff.additions, diff.deletions), (1, 1));
    assert_eq!(diff.diff.len(), 3);
    assert_eq!(diff.diff[2].line.text, "fn edited_in_terminal() {}");
    let mut review = open_diff_for_viewer("sample.rs", &cwd, None, false);
    assert!(review.additions > 2);
    assert!(
        review
            .diff
            .iter()
            .any(|row| row.line.text == "fn edited_again_later() {}")
    );
    assert!(
        review
            .diff
            .iter()
            .any(|row| row.line.text == "// a neighboring edit")
    );
    diff.background = true;
    host.push(json!({"view": "viewer", "data": diff}).to_string());
    review.cwd = cwd.clone();
    review.requested_path = "sample.rs".into();
    review.background = true;
    host.push(json!({"view": "viewer", "data": review}).to_string());
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let rendered = frame(&mut host);
        if rendered.items.iter().any(
            |item| matches!(item, Item::Text { text, .. } if text == "fn edited_in_terminal() {}"),
        ) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the diff did not scroll to the linked change: {:?}",
            rendered
                .items
                .iter()
                .rev()
                .filter_map(|item| match item {
                    Item::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .take(16)
                .collect::<Vec<_>>(),
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    host.push(r#"{"clickLink":true}"#.into());
    frame(&mut host);
    let source = next_view();
    assert_eq!(source.mode, "file");
    assert!(source.lines[20_898].highlighted);
    assert_eq!(source.lines.len(), 21_000);
    git(&["rm", "-f", "sample.rs"]);
    let deleted = open_diff_for_viewer("sample.rs", &cwd, Some(20_900), true);
    assert_eq!(
        deleted.diff[deleted.diff_focus.unwrap()].line.old_num,
        Some(20_900)
    );
    let _ = std::fs::remove_dir_all(cwd);
}

#[test]
fn launcher_and_session_search_keep_the_tiling_until_selection() {
    use jsnir::editor::{EditorEvent, PointSelection};
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(APP.into(), "{}".into(), TextEngine::default(), move |raw| {
            let request: Value = serde_json::from_str(&raw).unwrap();
            let response = match request["type"].as_str() {
                Some("openTile") => "3".into(),
                Some("browseProject") => json!({
                    "path": "/project", "parent": "/", "ancestors": [], "folders": []
                })
                .to_string(),
                _ => "ok".into(),
            };
            tx.send(request).unwrap();
            Ok(response)
        })
        .unwrap();
    install_fonts(&mut host);
    install_images_on_host(&mut host, 1.);
    let mut tiling = Workspaces::new(1, TilingMode::Niri);
    tiling.focus_workspace_relative(1);
    tiling.open_pane(2, false);
    tiling.focus_session(1);
    let items = json!([
        { "id": 1, "title": "API session", "cli": "codex", "cwd": "/project/api", "alive": true },
        { "id": 2, "title": "Docs session", "cli": "claude", "cwd": "/project/docs", "alive": true }
    ]);
    let screen = json!({
        "cols": 80, "rowsCount": 1, "cursorRow": 0, "cursorCol": 0, "cursorVisible": false,
        "rows": [{"idx": 0, "plain": "test", "spans": [], "links": []}]
    });
    for (view, data) in [
        ("viewport", json!([640, 800])),
        ("workspace", json!("/project")),
        ("tiling", json!(tiling.snapshot())),
        ("config", json!(config::IcyConfig::default())),
        (
            "sessions",
            json!({"activeId": 1, "archivedCount": 0, "items": items}),
        ),
        ("screens", json!({"1": screen, "2": screen})),
        (
            "keybinds",
            json!({"items": keybinds::views(&Default::default())}),
        ),
    ] {
        host.push(json!({"view": view, "data": data}).to_string());
    }
    let key = |host: &mut Host, action: &str| {
        host.push(json!({"view": "modalKey", "data": action}).to_string());
        frame(host)
    };
    let label = |frame: &Frame, value: &str| {
        frame.items.iter().rev().find_map(|item| match item {
            Item::Text { text, rect, .. } if text == value => Some(*rect),
            _ => None,
        })
    };
    let click = |host: &mut Host, rect: [f32; 4]| {
        host.pointer_move(Some([rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.]));
        host.pointer_down(PointSelection::Place);
        host.pointer_up();
        frame(host)
    };
    let baseline = frame(&mut host);
    let plus = icons::load_static(1.)
        .into_iter()
        .find(|p| p.0 == "glyph:plus")
        .unwrap()
        .1;
    let plus = baseline
        .items
        .iter()
        .find_map(|item| match item {
            Item::Image { image, rect, .. }
                if image.rgba.as_ref() == plus && rect[0] > 224. && rect[1] < 52. =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("header plus");
    let launcher = click(&mut host, plus);
    assert!(label(&launcher, "Search or enter a URL").is_some());
    assert!(host.focused());
    let terminal = label(&launcher, "Terminal").unwrap();
    let open_file = label(&launcher, "Open File").unwrap();
    host.pointer_move(Some([open_file[0] + 4., open_file[1] + 4.]));
    let hovered = frame(&mut host);
    let row_fill = |label: [f32; 4]| {
        hovered.items.iter().find_map(|item| match item {
            Item::Fill { rect, clip, .. }
                if rect[2] > 200.
                    && rect[3] == 44.
                    && rect[0] <= label[0]
                    && rect[1] <= label[1]
                    && rect[1] + rect[3] >= label[1] + label[3] =>
            {
                Some((*rect, *clip))
            }
            _ => None,
        })
    };
    let (selected, list_clip) = row_fill(terminal).expect("selected row background");
    let (hover, _) = row_fill(open_file).expect("hovered row background");
    assert!((hover[1] - selected[1] - selected[3] - 4.).abs() < 0.1);
    let thumb = hovered
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fill { rect, .. }
                if rect[2] == 8.
                    && (rect[0] + rect[2] + 2. - list_clip[0] - list_clip[2]).abs() < 0.1 =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("launcher scrollbar thumb");
    assert!((thumb[0] - hover[0] - hover[2] - 4.).abs() < 0.1);
    assert!(host.wheel_smooth(40.), "launcher accepts the mouse wheel");
    let scrolled = frame(&mut host);
    let scrolled_file = label(&scrolled, "Open File").unwrap();
    assert!(scrolled_file[1] < open_file[1]);
    assert!(host.wheel(12.), "launcher accepts a touchpad delta");
    let scrolled_file = label(&frame(&mut host), "Open File").unwrap();
    host.push(json!({"view": "screen", "data": screen}).to_string());
    let updated = frame(&mut host);
    assert_eq!(label(&updated, "Open File").unwrap(), scrolled_file);
    let cancelled = click(&mut host, [0., 0., 1., 1.]);
    assert!(label(&cancelled, "Search or enter a URL").is_none());
    assert!(!host.focused());

    let sidebar_search = label(&cancelled, "Search").unwrap_or_else(|| {
        let visible: Vec<_> = cancelled
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Text { text, rect, .. } if rect[0] < 224. && rect[1] < 52. => {
                    Some((text, rect))
                }
                _ => None,
            })
            .collect();
        panic!("missing sidebar search label in {visible:?}");
    });
    let search = click(&mut host, sidebar_search);
    assert!(label(&search, "/project/docs").is_some());
    assert!(host.focused());
    host.edit(&[EditorEvent::Insert("claude docs".into())]);
    let matches = frame(&mut host);
    assert!(label(&matches, "/project/docs").is_some());
    assert!(label(&matches, "/project/api").is_none());
    host.edit(&[EditorEvent::Enter]);
    frame(&mut host);
    assert!(!host.focused());
    let requests: Vec<_> = rx.try_iter().collect();
    assert!(requests.contains(&json!({"type": "selectSession", "id": 2})));
    assert!(
        !requests
            .iter()
            .any(|r| r["type"] == "openTile" || r["type"] == "closeSession")
    );

    key(&mut host, "session.search");
    key(&mut host, "down");
    key(&mut host, "enter");
    assert!(
        rx.try_iter()
            .any(|r| r == json!({"type": "selectSession", "id": 2}))
    );
    key(&mut host, "session.search");
    let switched = key(&mut host, "panel.side");
    assert!(label(&switched, "Search or enter a URL").is_some());
    assert!(host.focused());
    let folder = key(&mut host, "enter");
    assert!(label(&folder, "open folder").is_some());
    assert!(host.focused());
    assert!(!rx.try_iter().any(|r| r["type"] == "openTile"));
    key(&mut host, "escape");
    key(&mut host, "panel.side");
    key(&mut host, "down");
    key(&mut host, "down");
    key(&mut host, "enter");
    assert!(rx.try_iter().any(|r| r["type"] == "openTile"));

    key(&mut host, "panel.side");
    host.edit(&[EditorEvent::Insert("expand column".into())]);
    assert!(label(&frame(&mut host), "Expand column").is_some());
    key(&mut host, "enter");
    assert!(
        rx.try_iter()
            .any(|r| r == json!({"type": "tilingAction", "action": "maximizeColumn"}))
    );
}

#[test]
fn fullscreen_fills_the_workspace_when_switching_tabs() {
    let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut manager = SessionManager::new(cwd.clone(), Arc::new(|| {}));
    manager.tiling.set_mode(TilingMode::Niri);
    manager.update_viewport(640., 800.);
    let ids = [false, false, true].map(|stack| manager.open_tile(stack, cwd.clone()));
    let tiled_size = (manager.cols, manager.rows);
    manager.toggle_maximize();
    let fullscreen_size = (manager.cols, manager.rows);
    assert!(fullscreen_size.0 > tiled_size.0 && fullscreen_size.1 > tiled_size.1);

    let mut host =
        Host::with_request_handler(APP.into(), "{}".into(), TextEngine::default(), |_| {
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    let items: Vec<_> = ids
        .iter()
        .map(|id| json!({"id": id, "title": format!("Pane {id}"), "cli": "codex", "cwd": "/project", "alive": true}))
        .collect();
    let screens: serde_json::Map<String, Value> = ids
        .iter()
        .map(|id| (id.to_string(), json!({
            "cols": 80, "rowsCount": 1, "cursorRow": 0, "cursorCol": 0, "cursorVisible": false,
            "rows": [{"idx": 0, "plain": format!("full pane {id}"), "spans": [], "links": []}]
        })))
        .collect();
    for (view, data) in [
        ("viewport", json!([640, 800])),
        (
            "sessions",
            json!({"activeId": ids[2], "archivedCount": 0, "items": items}),
        ),
        ("screens", json!(screens)),
    ] {
        host.push(json!({"view": view, "data": data}).to_string());
    }
    let mut full_width = None;
    for id in [ids[2], ids[0], ids[1], ids[2]] {
        assert_eq!(manager.active_id(), id);
        assert_eq!((manager.cols, manager.rows), fullscreen_size);
        host.push(json!({"view": "tiling", "data": manager.tiling_snapshot()}).to_string());
        let rendered = frame(&mut host);
        let panes: Vec<_> = rendered
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Text { text, clip, .. } if text.starts_with("full pane ") => {
                    Some((text, clip[2]))
                }
                _ => None,
            })
            .collect();
        assert_eq!(panes.len(), 1, "visible panes: {panes:?}");
        assert_eq!(panes[0].0, &format!("full pane {id}"));
        let width = *full_width.get_or_insert(panes[0].1);
        assert!(width > 350. && (panes[0].1 - width).abs() < 1.);
        assert!(manager.step_tab(1));
    }
    assert!(manager.select_session(ids[2]));
    manager.toggle_maximize();
    assert_eq!((manager.cols, manager.rows), tiled_size);
}

#[test]
fn overview_zooms_the_live_workspace_and_selects_without_changing_fullscreen_state() {
    let mut tiling = Workspaces::new(1, TilingMode::Niri);
    tiling.open_pane(2, false);
    tiling.open_pane(3, false);
    tiling.toggle_maximize();
    let snapshot = tiling.snapshot();
    assert!(snapshot.view_offset > 0.0);

    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(APP.into(), "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    let items: Vec<_> = (1..=3)
        .map(|id| json!({"id": id, "title": format!("Pane {id}"), "cli": "codex", "cwd": "/project", "alive": true}))
        .collect();
    let screen = |id: u64| {
        json!({
            "cols": 80, "rowsCount": 1, "cursorRow": 0, "cursorCol": 0, "cursorVisible": false,
            "rows": [{"idx": 0, "plain": format!("content {id}"), "spans": [], "links": []}]
        })
    };
    for (view, data) in [
        ("viewport", json!([640, 800])),
        ("tiling", json!(snapshot)),
        (
            "sessions",
            json!({"activeId": 3, "archivedCount": 0, "items": items}),
        ),
        (
            "screens",
            json!({"1": screen(1), "2": screen(2), "3": screen(3)}),
        ),
    ] {
        host.push(json!({"view": view, "data": data}).to_string());
    }
    let zooms = |frame: &Frame, name: &str| {
        frame
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Text { text, zoom, .. } if text == name => Some(*zoom),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let tiled = frame(&mut host);
    assert!(tiled.items.iter().any(|item| matches!(item,
        Item::Text { text, rect, .. } if text == "Pane 3" && rect[1] < 52.
    )));
    assert_eq!(zooms(&tiled, "content 3"), vec![1.0]);

    let key = |host: &mut Host, action: &str| {
        host.push(json!({"view": "modalKey", "data": action}).to_string());
        frame(host)
    };
    // Every frame of the zoom draws the window exactly once: nothing fades over a copy of it.
    host.push(json!({"view": "modalKey", "data": "view.overview"}).to_string());
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(400) {
        host.poll();
        let copies = zooms(&host.frame(640., 800., 1., 4096), "content 3");
        assert!(copies.len() <= 1, "{copies:?}");
        std::thread::sleep(Duration::from_millis(8));
    }
    let overview = frame(&mut host);
    assert!(!overview.items.iter().any(|item| matches!(item,
        Item::Text { text, rect, .. } if text.starts_with("Pane ") && rect[1] < 52.
    )));
    // niri's overview zooms the live window out: one copy of it, at half size.
    let zoomed = zooms(&overview, "content 3");
    assert_eq!(zoomed.len(), 1, "{zoomed:?}");
    assert!((zoomed[0] - 0.5).abs() < 0.01, "{zoomed:?}");

    key(&mut host, "left");
    key(&mut host, "enter");
    let requests: Vec<_> = rx.try_iter().collect();
    assert!(requests.contains(&json!({"type": "selectSession", "id": 2})));
    assert!(requests.contains(&json!({"type": "overview", "on": false})));
    assert!(!requests.iter().any(|r| r["type"] == "toggleMaximize"));

    key(&mut host, "view.overview");
    let closed = key(&mut host, "view.overview");
    let requests: Vec<_> = rx.try_iter().collect();
    assert!(requests.contains(&json!({"type": "overview", "on": false})));
    assert!(!requests.iter().any(|r| r["type"] == "toggleMaximize"));
    assert_eq!(zooms(&closed, "content 3"), vec![1.0]);
}

/// Every workspace is drawn once, live, in a vertical stack: only the active one fills the area,
/// and the overview shows each of them below the previous one at niri's zoom.
#[test]
fn overview_stacks_every_workspace_once() {
    let mut tiling = Workspaces::new(1, TilingMode::Niri);
    tiling.open_pane(2, false);
    assert!(tiling.move_to_workspace(2, 1, false));
    tiling.focus_workspace(0);
    assert_eq!(tiling.workspace_of(2), Some(1));

    let mut host =
        Host::with_request_handler(APP.into(), "{}".into(), TextEngine::default(), |_| {
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    let screen = |id: u64| {
        json!({
            "cols": 80, "rowsCount": 1, "cursorRow": 0, "cursorCol": 0, "cursorVisible": false,
            "rows": [{"idx": 0, "plain": format!("content {id}"), "spans": [], "links": []}]
        })
    };
    for (view, data) in [
        ("viewport", json!([640, 800])),
        ("tiling", json!(tiling.snapshot())),
        (
            "sessions",
            json!({"activeId": 1, "archivedCount": 0, "items": [
                {"id": 1, "title": "Pane 1", "cli": "codex", "cwd": "/p", "alive": true},
                {"id": 2, "title": "Pane 2", "cli": "codex", "cwd": "/p", "alive": true}]}),
        ),
        ("screens", json!({"1": screen(1), "2": screen(2)})),
    ] {
        host.push(json!({"view": view, "data": data}).to_string());
    }
    let texts = |frame: &Frame, name: &str| {
        frame
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Text {
                    text, rect, zoom, ..
                } if text == name => Some((*rect, *zoom)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let tiled = frame(&mut host);
    assert_eq!(texts(&tiled, "content 1").len(), 1);
    assert!(texts(&tiled, "content 2").is_empty());

    host.push(json!({"view": "modalKey", "data": "view.overview"}).to_string());
    let overview = frame(&mut host);
    let (first, second) = (texts(&overview, "content 1"), texts(&overview, "content 2"));
    assert_eq!((first.len(), second.len()), (1, 1), "{first:?} {second:?}");
    assert!((first[0].1 - 0.5).abs() < 0.01 && (second[0].1 - 0.5).abs() < 0.01);
    assert!(
        second[0].0[1] > first[0].0[1] + 100.,
        "{first:?} {second:?}"
    );

    // Dropped on the first workspace, the window remounts in that workspace's layer (`frame`
    // fails on a rejected batch, ARCHITECTURE.md JSNIR gap 8).
    assert!(tiling.move_to_workspace(2, 0, false));
    host.push(json!({"view": "tiling", "data": tiling.snapshot()}).to_string());
    let moved = frame(&mut host);
    let (first, second) = (texts(&moved, "content 1"), texts(&moved, "content 2"));
    assert_eq!((first.len(), second.len()), (1, 1), "{first:?} {second:?}");
    assert!(
        (second[0].0[1] - first[0].0[1]).abs() < 1.,
        "{first:?} {second:?}"
    );
}

#[test]
fn maximize_crossfades_prev_and_next_surfaces_then_settles_to_one() {
    let mut tiling = Workspaces::new(1, TilingMode::Niri);
    tiling.open_pane(2, false);

    let mut host =
        Host::with_request_handler(APP.into(), "{}".into(), TextEngine::default(), |_| {
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    let screen = |id: u64| {
        json!({
            "cols": 80, "rowsCount": 1, "cursorRow": 0, "cursorCol": 0, "cursorVisible": false,
            "rows": [{"idx": 0, "plain": format!("content {id}"), "spans": [], "links": []}]
        })
    };
    for (view, data) in [
        ("viewport", json!([640, 800])),
        ("tiling", json!(tiling.snapshot())),
        (
            "sessions",
            json!({"activeId": 2, "archivedCount": 0, "items": [
                {"id": 1, "title": "Pane 1", "cli": "codex", "cwd": "/p", "alive": true},
                {"id": 2, "title": "Pane 2", "cli": "codex", "cwd": "/p", "alive": true}]}),
        ),
        ("screens", json!({"1": screen(1), "2": screen(2)})),
    ] {
        host.push(json!({"view": view, "data": data}).to_string());
    }

    let pane_surfaces = |frame: &Frame, needle: &str| {
        let mut stack = Vec::<f32>::new();
        let mut found = Vec::<Option<f32>>::new();
        for item in &frame.items {
            match item {
                Item::PushSurface { opacity, .. } => stack.push(*opacity),
                Item::PopSurface => {
                    stack.pop();
                }
                Item::Text { text, .. } if text == needle => {
                    found.push(stack.last().copied());
                }
                _ => {}
            }
        }
        found
    };

    let before = frame(&mut host);
    assert_eq!(pane_surfaces(&before, "content 2"), vec![None]);
    assert!(
        !before
            .items
            .iter()
            .any(|item| matches!(item, Item::PushSurface { .. }))
    );

    tiling.toggle_maximize();
    host.push(json!({"view": "tiling", "data": tiling.snapshot()}).to_string());

    let mut mid_opacities = None;
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(320) {
        host.poll();
        let rendered = host.frame(640., 800., 1., 4096);
        assert!(host.dev_error().is_none(), "{:?}", host.dev_error());
        let seen = pane_surfaces(&rendered, "content 2");
        if seen.len() == 2
            && let (Some(prev), Some(next)) = (seen[0], seen[1])
            && prev > 0.01
            && next > 0.01
        {
            mid_opacities = Some((prev, next));
            break;
        }
        std::thread::sleep(Duration::from_millis(12));
    }

    let (prev, next) = mid_opacities.expect("expected prev and next surfaces during maximize");
    assert!(
        (prev + next - 1.0).abs() < 1e-3,
        "crossfade opacities should sum to 1: prev={prev}, next={next}"
    );

    let settled = frame(&mut host);
    assert_eq!(pane_surfaces(&settled, "content 2"), vec![None]);
    assert!(
        !settled
            .items
            .iter()
            .any(|item| matches!(item, Item::PushSurface { .. })),
        "surfaces should be released after maximize settles"
    );
}

#[test]
fn directory_loader_lists_all_entries_and_keeps_missing_folders_empty() {
    let cwd = std::env::temp_dir().join(format!("icy-directory-{}", std::process::id()));
    std::fs::create_dir_all(cwd.join("folder")).unwrap();
    for i in 0..300 {
        std::fs::write(cwd.join(format!("file-{i:03}.rs")), "").unwrap();
    }
    let files = list_workspace_files(&cwd, "");
    assert_eq!(files.entries.len(), 301);
    assert_eq!(files.entries.first().unwrap().name, "folder");
    assert_eq!(files.entries.last().unwrap().name, "file-299.rs");
    assert!(
        list_workspace_files(&cwd, "removed-folder")
            .entries
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(cwd);
}

#[test]
fn open_tiles_do_not_highlight_sidebar_launchers() {
    let mut tiling = Workspaces::empty(TilingMode::Niri);
    tiling.open_pane(1, false);
    tiling.open_pane(2, false);
    for compact in [false, true] {
        let script = format!(
            r#"{APP}
            sidebarCompact.value = {compact};
            tileSpecs.value = {{1: {{kind: 'review', path: '', cwd: '/project'}}, 2: {{kind: 'metrics', path: ''}}}};
            tilingState.value = {};
            mount(Sidebar);
        "#,
            serde_json::to_string(&tiling.snapshot()).unwrap()
        );
        let mut host =
            Host::with_request_handler(script, "{}".into(), TextEngine::default(), |_| {
                Ok("ok".into())
            })
            .unwrap();
        install_fonts(&mut host);
        let marker = [1, 2, 3, 255];
        host.set_image("glyph:git-branch".into(), marker.to_vec(), [1, 1]);
        host.set_image("glyph:dashboard".into(), marker.to_vec(), [1, 1]);
        let rendered = frame(&mut host);
        let icons: Vec<_> = rendered
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Image { image, rect, .. } if image.rgba.as_ref() == marker => Some(*rect),
                _ => None,
            })
            .collect();
        assert_eq!(icons.len(), 2);
        for icon in icons {
            let x = icon[0] + icon[2] / 2.;
            let y = icon[1] + icon[3] / 2.;
            assert!(!rendered.items.iter().any(|item| matches!(item,
                Item::Fill {rect, top: [31, 31, 34, 255], ..}
                    if x >= rect[0] && x < rect[0] + rect[2] && y >= rect[1] && y < rect[1] + rect[3]
            )), "an open tile left its launcher highlighted (compact={compact})");
        }
    }
}

#[test]
fn code_preview_loader_returns_windowed_file_and_diff_snippets() {
    let cwd = std::env::temp_dir().join(format!("icy-code-preview-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cwd);
    std::fs::create_dir_all(&cwd).unwrap();

    let source_path = cwd.join("demo.rs");
    let lines: Vec<String> = (1..=40)
        .map(|i| format!("pub fn line_{i}() -> usize {{ {i} }}\n"))
        .collect();
    std::fs::write(&source_path, lines.concat()).unwrap();
    std::fs::write(cwd.join("blob.bin"), b"abc\0def").unwrap();

    let file_preview = build_code_preview(CodePreviewRequest {
        key: "file-25".into(),
        path: "demo.rs".into(),
        cwd: cwd.clone(),
        diff: false,
        line: Some(25),
        old_line: false,
        edit: None,
    });
    assert!(file_preview.error.is_none());
    assert_eq!(file_preview.mode, "file");
    assert_eq!(file_preview.display_path, "demo.rs");
    assert_eq!(file_preview.total_lines, 40);
    assert!(file_preview.truncated);
    assert_eq!(file_preview.lines.len(), CODE_PREVIEW_LINES);
    assert_eq!(file_preview.lines[0].num, 22);
    assert!(file_preview.lines[3].highlighted);
    assert_eq!(file_preview.lines[3].num, 25);
    assert!(!file_preview.lines[3].highlights.is_empty());

    let binary_preview = build_code_preview(CodePreviewRequest {
        key: "bin".into(),
        path: "blob.bin".into(),
        cwd: cwd.clone(),
        diff: false,
        line: None,
        old_line: false,
        edit: None,
    });
    assert_eq!(binary_preview.error.as_deref(), Some("Binary file"));

    let edit = links::EditPreview {
        lines: vec![
            DiffLine {
                kind: git::DiffKind::Ctx,
                old_num: Some(24),
                new_num: Some(24),
                text: "pub fn line_24() -> usize { 24 }".into(),
            },
            DiffLine {
                kind: git::DiffKind::Del,
                old_num: Some(25),
                new_num: None,
                text: "pub fn line_25() -> usize { 25 }".into(),
            },
            DiffLine {
                kind: git::DiffKind::Add,
                old_num: None,
                new_num: Some(25),
                text: "pub fn updated_25() -> usize { 250 }".into(),
            },
        ],
        counts: Some(git::EditCounts {
            additions: 1,
            deletions: 1,
        }),
    };
    let diff_preview = build_code_preview(CodePreviewRequest {
        key: "diff-25".into(),
        path: "demo.rs".into(),
        cwd: cwd.clone(),
        diff: true,
        line: Some(25),
        old_line: false,
        edit: Some(edit),
    });
    assert!(diff_preview.error.is_none());
    assert_eq!(diff_preview.mode, "diff");
    assert!(diff_preview.single_edit);
    assert_eq!((diff_preview.additions, diff_preview.deletions), (1, 1));
    assert_eq!(diff_preview.diff.len(), 3);
    assert_eq!(diff_preview.diff_focus, Some(2));
    assert!(!diff_preview.diff[2].highlights.is_empty());

    let _ = std::fs::remove_dir_all(cwd);
}

#[test]
fn code_preview_card_renders_file_and_diff_and_opens_full_panel_on_click() {
    use jsnir::editor::PointSelection;

    let file_link = json!({
        "kind": "file",
        "target": "src/demo.rs",
        "line": 25,
        "col": null,
        "resolved": true,
        "image": false
    });
    let diff_link = json!({
        "kind": "diff",
        "target": "src/demo.rs",
        "line": 25,
        "col": null,
        "oldLine": false,
        "edit": {
            "lines": [
                {"kind": "del", "oldNum": 25, "newNum": null, "text": "let old = 1;"},
                {"kind": "add", "oldNum": null, "newNum": 25, "text": "let updated = 2;"}
            ],
            "counts": {"additions": 1, "deletions": 1}
        },
        "resolved": true,
        "image": false
    });

    let script = format!(
        r#"{APP}
        const activeLink = signal({file_link});
        const fileKey = codePreviewKey({file_link}, 1);
        const diffKey = codePreviewKey({diff_link}, 1);
        receiveCodePreview({{
            key: fileKey,
            mode: "file",
            displayPath: "src/demo.rs",
            targetLine: 25,
            totalLines: 40,
            truncated: true,
            additions: 0,
            deletions: 0,
            lines: [
                {{ num: 24, text: "// before" }},
                {{ num: 25, text: "pub fn preview_target() {{}}", highlighted: true }}
            ]
        }});
        receiveCodePreview({{
            key: diffKey,
            mode: "diff",
            displayPath: "src/demo.rs",
            targetLine: 25,
            singleEdit: true,
            totalLines: 2,
            truncated: false,
            diffFocus: 1,
            additions: 1,
            deletions: 1,
            diff: [
                {{ kind: "del", oldNum: 25, newNum: null, text: "let old = 1;" }},
                {{ kind: "add", oldNum: null, newNum: 25, text: "let updated = 2;" }}
            ]
        }});
        onHost((message) => {{
            if (message.showDiff) activeLink.value = {diff_link};
        }});
        mount(() => CodePreviewCard({{
            link: () => activeLink.value,
            sessionId: () => 1,
            onOpen: () => openLink(activeLink.value, 1),
        }}));
        "#
    );

    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);

    let has_text = |rendered: &Frame, needle: &str| {
        rendered
            .items
            .iter()
            .any(|item| matches!(item, Item::Text { text, .. } if text == needle))
    };

    let file_frame = frame(&mut host);
    assert!(has_text(&file_frame, "src/demo.rs:25"));
    assert!(has_text(&file_frame, "pub fn preview_target() {}"));
    assert!(has_text(&file_frame, "+38 more lines"));
    assert!(has_text(&file_frame, "Click to open file panel"));

    host.pointer_move(Some([120., 40.]));
    host.pointer_down(PointSelection::Place);
    host.pointer_up();
    frame(&mut host);
    assert!(
        rx.try_iter()
            .any(|req| req["type"] == "open" && req["path"] == "src/demo.rs" && req["line"] == 25)
    );

    host.push(r#"{"showDiff":true}"#.into());
    let diff_frame = frame(&mut host);
    assert!(has_text(&diff_frame, "src/demo.rs"));
    assert!(has_text(&diff_frame, "let updated = 2;"));
    assert!(has_text(&diff_frame, "+1"));
    assert!(has_text(&diff_frame, "-1"));
    assert!(has_text(&diff_frame, "single edit"));
    assert!(has_text(&diff_frame, "Click to open diff panel"));

    host.pointer_down(PointSelection::Place);
    host.pointer_up();
    frame(&mut host);
    assert!(
        rx.try_iter()
            .any(|req| req["type"] == "openDiff" && req["path"] == "src/demo.rs")
    );
}

#[test]
fn pinned_doc_tile_stays_open_while_next_file_opens_beside_it() {
    let script = format!(
        r#"{APP}
        const assert = (ok, message) => {{ if (!ok) throw new Error(message); }};
        const setPanes = (ids, focused) => {{
            tilingState.value = {{
                ...tilingState.value,
                focusedSession: focused,
                columns: ids.map((id, i) => ({{
                    id: i + 1, panes: [id], heights: [1], x: i * 0.5, width: 0.5, expanded: false, visible: true
                }})),
                workspaces: [{{ panes: ids, focused, columns: [], viewOffset: 0 }}],
            }};
        }};
        workspacePath.value = "/project";
        setPanes([1], 1);
        tileSpecs.value = {{ 1: {{ kind: "file", path: "a.rs", cwd: "/project" }} }};

        // Unpinned preview is reused for the next file.
        openDocTile({{ kind: "file", path: "b.rs", cwd: "/project" }});
        assert(tileSpecs.value[1].path === "b.rs", "expected unpinned preview to be reused");

        // Pinning keeps b.rs intact; c.rs opens a new tile beside it.
        setTile(1, {{ ...tileSpecs.value[1], pinned: true }}, false);
        openDocTile({{ kind: "file", path: "c.rs", cwd: "/project" }});
        setPanes([1, 2], 2);
        assert(tileSpecs.value[1].path === "b.rs" && tileSpecs.value[1].pinned, "pinned tile must remain untouched");
        assert(tileSpecs.value[2].path === "c.rs" && !tileSpecs.value[2].pinned, "new file should open in tile 2");

        // Next file reuses unpinned tile 2 while tile 1 stays pinned.
        openDocTile({{ kind: "file", path: "d.rs", cwd: "/project" }});
        assert(tileSpecs.value[1].path === "b.rs" && tileSpecs.value[1].pinned, "tile 1 must stay pinned");
        assert(tileSpecs.value[2].path === "d.rs", "tile 2 should be reused");

        // Re-opening b.rs focuses tile 1 and preserves its pinned state.
        openDocTile({{ kind: "file", path: "b.rs", cwd: "/project" }});
        assert(tileSpecs.value[1].path === "b.rs" && tileSpecs.value[1].pinned, "refocused tile must remain pinned");
        mount(() => Text({{ children: "passed" }}));
        "#
    );
    let (tx, rx) = mpsc::channel();
    let mut next_tile_id = 2u64;
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            let value = serde_json::from_str::<Value>(&raw).unwrap();
            let reply = if value["type"] == "openTile" {
                let id = next_tile_id;
                next_tile_id += 1;
                id.to_string()
            } else {
                "ok".into()
            };
            tx.send(value).unwrap();
            Ok(reply)
        })
        .unwrap();
    frame(&mut host);
    let requests: Vec<_> = rx.try_iter().collect();
    assert_eq!(
        requests.iter().filter(|r| r["type"] == "openTile").count(),
        1
    );
    assert!(
        requests
            .iter()
            .any(|r| r["type"] == "selectSession" && r["id"] == 1)
    );
}

#[test]
fn open_in_menu_lists_detected_editors_and_dispatches_open_in_editor() {
    use jsnir::editor::PointSelection;

    let script = format!(
        r#"{APP}
        workspacePath.value = "/project";
        configState.value = {{ ...configState.value, preferredEditor: "cursor" }};
        externalEditorsState.value = [
            {{ id: "vscode", name: "VS Code" }},
            {{ id: "cursor", name: "Cursor" }},
        ];
        tileSpecs.value = {{
            1: {{ kind: "diff", path: "src/app.rs", cwd: "/project" }},
        }};
        docCache.value = {{
            [docKey(tileSpecs.value[1])]: {{
                cwd: "/project",
                open: true,
                path: "/project/src/app.rs",
                displayPath: "src/app.rs",
                mode: "diff",
                lines: [],
                diff: [
                    {{ kind: "ctx", oldNum: 10, newNum: 10, text: "fn main() {{" }},
                    {{ kind: "add", oldNum: null, newNum: 11, text: "    run();" }},
                ],
                additions: 1,
                deletions: 0,
            }},
        }};
        mount(() => TilePane({{ id: () => 1, focused: () => true, multiPane: () => true }}));
        "#
    );
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    install_images_on_host(&mut host, 1.0);

    // Hover the tile header so action buttons mount, then click the "Open in…" button.
    let external_link = [1, 2, 3, 255];
    host.set_image("glyph:external-link".into(), external_link.to_vec(), [1, 1]);
    host.pointer_move(Some([120., 15.]));
    let hovered = frame(&mut host);
    let ext_rect = hovered
        .items
        .iter()
        .find_map(|item| match item {
            Item::Image { image, rect, .. } if image.rgba.as_ref() == external_link => Some(*rect),
            _ => None,
        })
        .expect("Open in button icon should be visible on hover");
    let center = |r: [f32; 4]| [r[0] + r[2] * 0.5, r[1] + r[3] * 0.5];
    host.pointer_move(Some(center(ext_rect)));
    host.pointer_down(PointSelection::Place);
    host.pointer_up();
    let opened = frame(&mut host);

    let text_rect = |f: &Frame, label: &str| {
        f.items.iter().find_map(|item| match item {
            Item::Text { text, rect, .. } if text == label => Some(*rect),
            _ => None,
        })
    };
    let cursor_rect = text_rect(&opened, "Cursor").expect("Cursor row in Open in menu");
    let vscode_rect = text_rect(&opened, "VS Code").expect("VS Code row in Open in menu");
    assert!(
        cursor_rect[1] < vscode_rect[1],
        "preferred editor should appear first in Open in menu"
    );
    assert!(text_rect(&opened, "System default").is_some());
    assert!(text_rect(&opened, "Show in file manager").is_some());
    assert!(text_rect(&opened, "Copy path").is_some());

    // Move pointer into the menu (outside the tile header) and click Cursor.
    host.pointer_move(Some(center(cursor_rect)));
    host.pointer_down(PointSelection::Place);
    host.pointer_up();
    frame(&mut host);

    let req = rx
        .try_iter()
        .find(|r| r["type"] == "openInEditor")
        .expect("openInEditor request");
    assert_eq!(req["editorId"], "cursor");
    assert_eq!(req["path"], "/project/src/app.rs");
    assert_eq!(req["line"], 11);
}

#[test]
fn git_graph_tab_requests_the_log_and_draws_lanes_refs_and_subjects() {
    let script = format!(
        r#"{APP}
        reviewMode.value = 'graph';
        mount(() => ReviewView({{cwd: '/project'}}));
    "#
    );
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    frame(&mut host);
    let request = rx
        .try_iter()
        .find(|r| r["type"] == "gitGraph")
        .expect("graph requested on open");
    assert_eq!(request["cwd"], "/project");
    assert!(request["lane"].as_f64().unwrap() > 0. && request["row"].as_f64().unwrap() > 0.);

    let marker = [9, 8, 7, 255];
    host.set_image("ggraph:test".into(), marker.to_vec(), [1, 1]);
    host.push(
        json!({"view": "gitGraph", "data": {"cwd": "/project", "width": 2, "limit": 500, "rows": [
            {"hash": "m1", "short": "m1", "subject": "Merge feature", "author": "a", "time": 0,
             "refs": [{"kind": "head", "name": "main"}], "node": 0, "color": 0, "merge": true,
             "layers": [{"color": 0, "key": "ggraph:test"}]},
            {"hash": "f1", "short": "f1", "subject": "feat: lanes", "author": "a", "time": 0,
             "refs": [], "node": 1, "color": 1, "merge": false, "layers": []}
        ]}})
        .to_string(),
    );
    let drawn = frame(&mut host);
    for text in ["Merge feature", "feat: lanes", "main"] {
        assert!(
            drawn
                .items
                .iter()
                .any(|item| matches!(item, Item::Text { text: t, .. } if t == text)),
            "{text}"
        );
    }
    assert!(drawn.items.iter().any(|item| matches!(item,
        Item::Image { image, .. } if image.rgba.as_ref() == marker)));
}

#[test]
fn graph_layers_draw_inside_their_lane() {
    use icy_core::git_graph::Seg;
    let through = [Seg {
        x1: 1,
        y1: 0,
        x2: 1,
        y2: 2,
        color: 0,
    }];
    let (rgba, [w, h]) = icons::rasterize_graph_layer(&through, 2, 14., 28., 2., 1.).unwrap();
    assert_eq!([w, h], [28, 28]);
    let alpha = |x: u32, y: u32| rgba[((y * w + x) * 4 + 3) as usize];
    assert!(alpha(21, 14) > 0, "lane 1 centre is drawn");
    assert_eq!(alpha(7, 14), 0, "lane 0 stays empty");

    // The repo itself lays out without panicking and keeps every row inside the width.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Some(commits) = icy_core::git_graph::log(root, 200) {
        let graph = icy_core::git_graph::layout(commits);
        assert!(graph.rows.iter().all(|r| {
            r.node < graph.width
                && r.segs
                    .iter()
                    .all(|s| s.x1 < graph.width && s.x2 < graph.width)
        }));
    }
}

#[test]
fn theme_icons_replace_glyphs_and_cli_logos_by_name() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="24" height="24" fill="red"/></svg>"#;
    let dir = std::env::temp_dir().join(format!("icy-theme-icons-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("folder.svg"), svg).unwrap();
    let icons = std::collections::BTreeMap::from([
        (
            "folder".to_string(),
            dir.join("folder.svg").display().to_string(),
        ),
        ("cli:claude".to_string(), svg.to_string()),
        ("not-a-glyph".to_string(), svg.to_string()),
        (
            "search".to_string(),
            dir.join("missing.svg").display().to_string(),
        ),
    ]);
    let pictures = icons::theme_icon_pictures(&icons, 1.0);
    let names: Vec<_> = pictures.iter().map(|p| p.0.as_str()).collect();
    assert_eq!(names, vec!["cli:claude", "glyph:folder"]);
    let folder = &pictures[1];
    // Glyphs stay white masks so the theme tints them; logos keep their colour.
    assert!(folder.1.chunks(4).any(|px| px == [255, 255, 255, 255]));
    assert!(
        pictures[0]
            .1
            .chunks(4)
            .any(|px| px[0] == 255 && px[1] == 0 && px[3] == 255)
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Time until the app thread has drained its work and no animation is running (capped at 1 s),
/// counting every frame drawn meanwhile. `false` when it never went idle.
fn settle(host: &mut Host) -> (f64, bool) {
    let start = Instant::now();
    loop {
        host.poll();
        let frame = host.frame(1600., 1000., 1., 4096);
        assert!(host.dev_error().is_none(), "{:?}", host.dev_error());
        if !host.busy() && !frame.animating {
            return (start.elapsed().as_secs_f64() * 1000., true);
        }
        if start.elapsed() >= Duration::from_secs(1) {
            return (1000., false);
        }
        std::thread::sleep(Duration::from_micros(200));
    }
}

/// Frame cost of the full shell (QuickJS + layout, no GPU). Run in release for real numbers:
/// `cargo test --release -p icy-app perf_ -- --ignored --nocapture`.
#[test]
#[ignore]
fn perf_tab_switch_and_alt_hold() {
    let mut tiling = Workspaces::empty(TilingMode::Niri);
    for id in 1..=8 {
        tiling.open_pane(id, false);
    }
    let clis = ["claude", "codex", "opencode", "terminal"];
    let items: Vec<_> = (1..=8u64)
        .map(|id| {
            json!({
                "id": id, "title": format!("agent {id}"), "cwd": format!("/proj/{}", id % 3),
                "cli": clis[(id % 4) as usize],
                "alive": true, "status": if id % 2 == 0 { "working" } else { "idle" },
            })
        })
        .collect();
    let mut host = Host::with_request_handler(
        format!("{APP}\nmount(IcyApp);"),
        "{}".into(),
        TextEngine::default(),
        |_| Ok("ok".into()),
    )
    .unwrap();
    install_fonts(&mut host);
    install_images_on_host(&mut host, 1.0);
    let measure = |host: &mut Host, label: &str, step: &mut dyn FnMut(&mut Host, usize)| {
        let mut times = Vec::new();
        let mut unsettled = 0;
        for i in 0..40 {
            step(host, i);
            let (ms, settled) = settle(host);
            times.push(ms);
            if !settled {
                unsettled += 1;
            }
        }
        times.sort_by(f64::total_cmp);
        eprintln!(
            "{label:<22} median {:>7.2} ms   p95 {:>7.2} ms   max {:>7.2} ms   never idle {unsettled}/40",
            times[times.len() / 2],
            times[times.len() * 95 / 100],
            times[times.len() - 1]
        );
    };
    host.push(json!({"view": "sessions", "data": {"activeId": 1, "items": items}}).to_string());
    host.push(json!({"view": "tiling", "data": tiling.snapshot()}).to_string());
    measure(&mut host, "first frames", &mut |_, _| {});
    measure(&mut host, "idle frame", &mut |_, _| {});
    measure(&mut host, "tab switch", &mut |host, i| {
        tiling.focus_direction(if (i / 4) % 2 == 0 {
            FocusDir::Right
        } else {
            FocusDir::Left
        });
        let active = tiling.snapshot();
        host.push(json!({"view": "tiling", "data": active}).to_string());
        host.push(
            json!({"view": "sessions", "data": {"activeId": (i % 8) + 1, "items": items}})
                .to_string(),
        );
    });
    measure(&mut host, "alt hold toggle", &mut |host, i| {
        host.push(json!({"view": "agentDigitsHeld", "data": i % 2 == 0}).to_string());
    });
    measure(&mut host, "sessions status push", &mut |host, i| {
        let mut items = items.clone();
        items[i % 8]["status"] = json!(if i % 2 == 0 { "idle" } else { "working" });
        host.push(json!({"view": "sessions", "data": {"activeId": 1, "items": items}}).to_string());
    });
}

/// Cost of one bridge screen push with full terminals: snapshot + JSON on the host, then the
/// TSX frame that consumes it. `cargo test --release -p icy-app perf_ -- --ignored --nocapture`.
#[test]
#[ignore]
fn perf_screen_push_with_full_terminals() {
    let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    let mut manager = SessionManager::new(cwd.clone(), Arc::clone(&notify));
    let screenful: String = (0..60)
        .map(|i| format!("{i:03} {}\n", "lorem ipsum dolor sit amet ".repeat(5)))
        .collect();
    for _ in 0..8 {
        let id = manager.allocate_session_id();
        let (program, args) = platform::echo_command(&screenful);
        let session = Session::spawn_command(
            id,
            "agent",
            cwd.clone(),
            &program,
            &args,
            160,
            50,
            Arc::clone(&notify),
        )
        .unwrap();
        manager.place_session(session, false);
    }
    std::thread::sleep(Duration::from_millis(800));
    let time = |label: &str, runs: usize, f: &mut dyn FnMut()| {
        let start = Instant::now();
        for _ in 0..runs {
            f();
        }
        eprintln!(
            "{label:<34} {:>7.3} ms",
            start.elapsed().as_secs_f64() * 1000. / runs as f64
        );
    };
    let visible = manager.visible_screens().len();
    eprintln!("visible panes: {visible} of 8");
    time("snapshot focused screen", 50, &mut || {
        let _ = manager.context_terminal().map(build_screen_payload);
    });
    time("snapshot visible screens", 50, &mut || {
        let _ = manager.visible_screens();
    });
    let screens = manager.visible_screens();
    let mut json = String::new();
    time("serialize visible screens (json)", 50, &mut || {
        json = serde_json::to_string(&screens).unwrap();
    });
    eprintln!("screens payload size: {} KiB", json.len() / 1024);
    manager.overview = true;
    let all = manager.visible_screens();
    let all_json = serde_json::to_string(&all).unwrap();
    eprintln!(
        "overview payload size (8 screens): {} KiB",
        all_json.len() / 1024
    );
    manager.overview = false;

    let mut host = Host::with_request_handler(
        format!("{APP}\nmount(IcyApp);"),
        "{}".into(),
        TextEngine::default(),
        |_| Ok("ok".into()),
    )
    .unwrap();
    install_fonts(&mut host);
    host.push(json!({"view": "sessions", "data": manager.sessions_payload()}).to_string());
    host.push(json!({"view": "tiling", "data": manager.tiling_snapshot()}).to_string());
    let focused =
        serde_json::to_string(&manager.context_terminal().map(build_screen_payload)).unwrap();
    let mut frames = Vec::new();
    for i in 0..30 {
        // Alternate payloads so the UI really diffs new rows each push.
        let payload = if i % 2 == 0 {
            &json
        } else {
            &serde_json::to_string(&manager.visible_screens()).unwrap()
        };
        host.push(format!(r#"{{"view":"screen","data":{focused}}}"#));
        host.push(format!(r#"{{"view":"screens","data":{payload}}}"#));
        frames.push(settle(&mut host).0);
    }
    frames.sort_by(f64::total_cmp);
    eprintln!(
        "tsx frame after screen push        median {:>7.2} ms  max {:>7.2} ms",
        frames[15], frames[29]
    );
}

/// Process CPU time (user + system) from /proc, in ms. Linux-only measurement helper.
fn process_cpu_ms() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = stat
        .rsplit(')')
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect();
    let ticks: f64 = fields.get(11).and_then(|v| v.parse().ok()).unwrap_or(0.)
        + fields.get(12).and_then(|v| v.parse().ok()).unwrap_or(0.);
    ticks * 10. // USER_HZ is 100 on Linux
}

/// CPU a working-agents UI burns while nothing else happens, drawing at 60 fps like the window.
/// `cargo test --release -p icy-app perf_ -- --ignored --nocapture --test-threads=1`
#[test]
#[ignore]
fn perf_idle_cpu_with_working_agents() {
    for working in [false, true] {
        let mut tiling = Workspaces::empty(TilingMode::Niri);
        for id in 1..=8 {
            tiling.open_pane(id, false);
        }
        let items: Vec<_> = (1..=8u64).map(|id| json!({
            "id": id, "title": format!("✳ Refactoring the agent sidebar {id}"), "cwd": format!("/proj/{}", id % 3),
            "cli": "claude", "alive": true, "status": if working && id % 2 == 0 { "working" } else { "idle" },
        })).collect();
        let mut host = Host::with_request_handler(
            format!("{APP}\nmount(IcyApp);"),
            "{}".into(),
            TextEngine::default(),
            |_| Ok("ok".into()),
        )
        .unwrap();
        install_fonts(&mut host);
        host.push(json!({"view": "sessions", "data": {"activeId": 1, "items": items}}).to_string());
        host.push(json!({"view": "tiling", "data": tiling.snapshot()}).to_string());
        settle(&mut host);
        let (cpu0, wall) = (process_cpu_ms(), Instant::now());
        let mut frames = 0;
        while wall.elapsed() < Duration::from_secs(2) {
            host.poll();
            let frame = host.frame(1600., 1000., 1., 4096);
            if frame.animating || host.busy() {
                frames += 1;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        let cpu = (process_cpu_ms() - cpu0) / wall.elapsed().as_secs_f64() / 10.;
        eprintln!(
            "{:<26} cpu {:>5.1}% of a core   animated frames {frames}",
            if working {
                "4 working agents"
            } else {
                "all idle"
            },
            cpu
        );
    }
}

#[test]
fn browser_tile_opens_draws_frames_and_shows_errors() {
    let script = format!(
        r#"{APP}
        viewportSize.value = [640, 800];
        mount(() => BrowserView({{id: 7, spec: {{kind: "browser", path: "", url: "http://localhost:3000"}}}}));
    "#
    );
    let (tx, rx) = mpsc::channel();
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    frame(&mut host);
    // The page is opened once the body has its size (after the layout settle delay).
    std::thread::sleep(Duration::from_millis(60));
    frame(&mut host);
    let open = rx
        .try_iter()
        .find(|r| r["type"] == "browser" && r["op"] == "open")
        .expect("page opened");
    assert_eq!(open["id"], 7);
    assert_eq!(open["url"], "http://localhost:3000");
    assert!(open["width"].as_f64().unwrap() > 0. && open["height"].as_f64().unwrap() > 0.);

    let pixels = vec![255; 16];
    host.set_image("browser:7".into(), pixels.clone(), [2, 2]);
    assert!(frame(&mut host).items.iter().any(|item| matches!(item,
        Item::Image { image, .. } if image.rgba.as_ref() == pixels.as_slice())));

    host.push(json!({"view": "browserState", "data": {"id": 7, "url": "http://localhost:3000", "title": "",
        "loading": false, "canBack": false, "canForward": false, "error": "Browser runtime not found"}}).to_string());
    assert!(frame(&mut host).items.iter().any(|item| matches!(item,
        Item::Text { text, .. } if text == "Browser runtime not found")));
}

#[test]
fn browser_addresses_resolve_on_the_host_and_each_url_opens_a_pane_that_closes_with_its_tile() {
    let (tx, rx) = mpsc::channel();
    let script = format!(
        r#"{APP}
        requestHost({{type: "paletteLabels", labels: launcherGroups("localhost:3000", "/project")
          .flatMap((group) => group.results.map((result) => result.label))}});
        openBrowser("http://localhost:3000");
    "#
    );
    let mut host =
        Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
            let value: Value = serde_json::from_str(&raw).unwrap();
            if value["type"] == "browserAddress" {
                let url =
                    icy_core::browser::normalize_url(value["text"].as_str().unwrap_or_default());
                return Ok(json!({"url": url}).to_string());
            }
            // Tile ids for new windows, so the browser pane gets id 9.
            if value["type"] == "openTile" {
                tx.send(value).unwrap();
                return Ok("9".into());
            }
            tx.send(value).unwrap();
            Ok("ok".into())
        })
        .unwrap();
    install_fonts(&mut host);
    frame(&mut host);
    let labels = rx
        .try_iter()
        .find(|r| r["type"] == "paletteLabels")
        .expect("palette labels");
    assert!(
        labels["labels"]
            .as_array()
            .unwrap()
            .contains(&json!("http://localhost:3000"))
    );
    assert!(rx.try_iter().any(|r| r["type"] == "openTile"));

    let mut tiles = Workspaces::empty(TilingMode::Niri);
    tiles.open_pane(9, false);
    host.push(json!({"view": "tiling", "data": tiles.snapshot()}).to_string());
    frame(&mut host);
    host.push(
        json!({"view": "tiling", "data": Workspaces::empty(TilingMode::Niri).snapshot()})
            .to_string(),
    );
    frame(&mut host);
    assert!(
        rx.try_iter()
            .any(|r| r["type"] == "browser" && r["op"] == "close" && r["id"] == 9)
    );
}

#[test]
fn browser_sidebar_opens_a_blank_tiled_pane_in_both_modes() {
    const GLOBE: [u8; 4] = [7, 11, 19, 255];
    for compact in [false, true] {
        let (tx, rx) = mpsc::channel();
        let script =
            format!("{APP}\nviewportSize.value = [640, 800]; sidebarCompact.value = {compact};");
        let mut host =
            Host::with_request_handler(script, "{}".into(), TextEngine::default(), move |raw| {
                let request: Value = serde_json::from_str(&raw).unwrap();
                let is_tile = request["type"] == "openTile";
                tx.send(request).unwrap();
                Ok(if is_tile { "9" } else { "ok" }.into())
            })
            .unwrap();
        install_fonts(&mut host);
        host.set_image("glyph:globe".into(), GLOBE.to_vec(), [1, 1]);
        let initial = frame(&mut host);
        let rect = initial
            .items
            .iter()
            .find_map(|item| match item {
                Item::Text { text, rect, .. } if !compact && text == "Browser" => Some(*rect),
                Item::Image { image, rect, .. } if compact && image.rgba.as_ref() == GLOBE => {
                    Some(*rect)
                }
                _ => None,
            })
            .expect("sidebar browser action");
        host.pointer_move(Some([rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.]));
        host.pointer_down(jsnir::editor::PointSelection::Place);
        host.pointer_up();
        frame(&mut host);
        assert_eq!(rx.try_iter().filter(|r| r["type"] == "openTile").count(), 1);

        let tiles = Workspaces::new(9, TilingMode::Niri);
        host.push(json!({"view": "tiling", "data": tiles.snapshot()}).to_string());
        frame(&mut host);
        std::thread::sleep(Duration::from_millis(60));
        let browser = frame(&mut host);
        let open = rx
            .try_iter()
            .find(|r| r["type"] == "browser" && r["op"] == "open")
            .expect("browser view mounted in the tiling");
        assert_eq!(open["id"], 9);
        assert_eq!(open["url"], "about:blank");
        assert!(
            host.focused(),
            "the blank browser focuses the address field"
        );
        let sidebar_width = if compact { 52. } else { 224. };
        assert!(browser.items.iter().any(|item| matches!(item,
            Item::Text { text, rect, .. } if text == "Browser" && rect[0] >= sidebar_width && rect[1] < 52.)));
    }
}

#[test]
fn terminal_frame_reuses_visible_context_and_restores_hidden_context() {
    let screen = |status: &str| ScreenPayload {
        meta: ScreenMeta {
            cols: 80,
            rows_count: 2,
            cursor_row: 1,
            cursor_col: 0,
            cursor_visible: false,
            display_offset: 0,
            images: Vec::new(),
        },
        rows: ["stable", status]
            .into_iter()
            .enumerate()
            .map(|(idx, plain)| RowPayload {
                idx,
                plain: plain.into(),
                spans: Vec::new(),
                links: Vec::new(),
            })
            .collect(),
    };
    let (tx, rx) = mpsc::channel();
    let mut host = Host::with_request_handler(
        format!(r#"{APP}
            mount(() => Text({{children: () => screenState.value.rows.map(row => row.plain).join(' ')}}));"#),
        "{}".into(),
        TextEngine::default(),
        move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap()).unwrap();
            Ok("ok".into())
        },
    ).unwrap();
    let visible = TerminalScreens {
        visible: BTreeMap::from([(1, screen("running"))]),
        context_id: Some(1),
        context_screen: None,
    };
    let hidden = TerminalScreens {
        visible: BTreeMap::new(),
        context_id: Some(1),
        context_screen: Some(screen("hidden")),
    };
    let restored = TerminalScreens {
        visible: BTreeMap::new(),
        context_id: Some(1),
        context_screen: Some(screen("updated")),
    };
    for (update, label) in [
        (visible.update_since(None, 1), "stable running"),
        (hidden.update_since(Some(&visible), 2), "stable hidden"),
        (restored.update_since(Some(&hidden), 3), "stable updated"),
        (visible.update_since(Some(&restored), 4), "stable running"),
    ] {
        let data = json!(update);
        if update.sequence == 1 {
            assert!(data["contextScreen"].is_null());
        } else if update.sequence == 2 {
            assert_eq!(data["contextScreen"]["rows"][0]["plain"], "stable");
        } else if update.sequence == 3 {
            assert!(data["contextScreen"]["rows"][0].is_null());
        }
        host.push(json!({"view": "terminalFrame", "data": data}).to_string());
        assert!(frame(&mut host).items.iter().any(|item| matches!(item,
            Item::Text { text, .. } if text == label)));
        assert!(
            rx.try_iter()
                .any(|request| request["type"] == "terminalFrameReceived"
                    && request["sequence"] == update.sequence)
        );
    }
}

/// A Claude-Code-like stream: a full coloured screen where only the spinner row changes per push.
/// Measures the app thread's CPU per push. `cargo test --release -p icy-app perf_spinner -- --ignored --nocapture`
#[test]
#[ignore]
fn perf_spinner_stream_only_reflows_changed_rows() {
    let body = |frame: usize| {
        let spinner = ["·", "✢", "✳", "✶", "✻", "✽"][frame % 6];
        let rows = (0..50)
            .map(|idx| {
                let text = if idx == 44 {
                    format!("{spinner} Thinking… ({frame}s · esc to interrupt)")
                } else {
                    format!(
                        "{idx:03} fn render_row_{idx}(ctx: &mut Context) -> Result<(), Error> {{"
                    )
                };
                let plain = format!("{text:<120}");
                let chars: Vec<_> = plain.chars().collect();
                let spans = chars
                    .chunks(30)
                    .enumerate()
                    .map(|(k, chunk)| SpanPayload {
                        text: chunk.iter().collect(),
                        col: (k * 30) as u16,
                        cells: 30,
                        fg: 0xc0caf5ff - k as u32 * 0x100,
                        bg: None,
                        bold: k == 0,
                        underline: false,
                        link: false,
                        image: false,
                        selected: false,
                        link_index: None,
                    })
                    .collect();
                RowPayload {
                    idx,
                    plain,
                    spans,
                    links: Vec::new(),
                }
            })
            .collect();
        TerminalScreens {
            visible: BTreeMap::from([(
                1,
                ScreenPayload {
                    meta: ScreenMeta {
                        cols: 160,
                        rows_count: 50,
                        cursor_row: 45,
                        cursor_col: 2,
                        cursor_visible: true,
                        display_offset: 0,
                        images: Vec::new(),
                    },
                    rows,
                },
            )]),
            context_id: Some(1),
            context_screen: None,
        }
    };
    let mut tiling = Workspaces::empty(TilingMode::Niri);
    tiling.open_pane(1, false);
    let mut host = Host::with_request_handler(
        format!("{APP}\nmount(IcyApp);"),
        "{}".into(),
        TextEngine::default(),
        |_| Ok("ok".into()),
    )
    .unwrap();
    install_fonts(&mut host);
    host.push(json!({"view": "sessions", "data": {"activeId": 1, "items": [
        {"id": 1, "title": "✳ Claude Code", "cwd": "/p", "cli": "claude", "alive": true, "status": "working"}]}}).to_string());
    host.push(json!({"view": "tiling", "data": tiling.snapshot()}).to_string());
    let mut previous = body(0);
    host.push(json!({"view": "terminalFrame", "data": previous.update_since(None, 0)}).to_string());
    settle(&mut host);
    let (cpu0, wall) = (process_cpu_ms(), Instant::now());
    let pushes = 120;
    for frame in 1..=pushes {
        let next = body(frame);
        host.push(json!({"view": "terminalFrame", "data": next.update_since(Some(&previous), frame as u64)}).to_string());
        previous = next;
        host.poll();
        let _ = host.frame(1600., 1000., 1., 4096);
        std::thread::sleep(Duration::from_millis(16));
    }
    let elapsed = wall.elapsed().as_secs_f64();
    let cpu = process_cpu_ms() - cpu0;
    eprintln!(
        "spinner stream: {:.1}% of a core, {:.2} ms CPU per push ({pushes} pushes in {elapsed:.2}s)",
        cpu / elapsed / 10.,
        cpu / pushes as f64
    );
}

/// Hovering the second of two links on one terminal row previews that link, anchored at it.
#[test]
fn terminal_hover_previews_the_link_under_the_pointer() {
    let (tx, rx) = mpsc::channel();
    let mut host = Host::with_request_handler(format!("{APP}\nmount(() => TerminalPane({{sessionId: () => 1, focused: () => true, multiPane: () => false}}));"),
        "{}".into(), TextEngine::default(), move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap()).unwrap();
            Ok("ok".into())
        }).unwrap();
    install_fonts(&mut host);
    let link = |target: &str| json!({"kind": "file", "target": target, "line": null, "col": null, "resolved": true, "image": false});
    let span = |text: &str, col: u16, link: Option<usize>| {
        let mut s = json!({"text": text, "col": col, "cells": text.len(), "fg": 0xffffffffu32, "bold": false, "underline": link.is_some(), "link": link.is_some()});
        if let Some(i) = link {
            s["linkIndex"] = json!(i);
        }
        s
    };
    let plain = "see README.md and DESIGN.md now";
    let screen = json!({"cols": 80, "rowsCount": 2, "cursorRow": 1, "cursorCol": 0, "cursorVisible": false, "rows": [
        {"idx": 0, "plain": plain, "links": [link("README.md"), link("DESIGN.md")], "spans": [
            span("see ", 0, None), span("README.md", 4, Some(0)), span(" and ", 13, None),
            span("DESIGN.md", 18, Some(1)), span(" now", 27, None)]},
        {"idx": 1, "plain": "", "links": [], "spans": []}]});
    host.push(json!({"view": "screens", "data": {"1": screen}}).to_string());
    let drawn = frame(&mut host);
    let design = drawn
        .items
        .iter()
        .find_map(|item| match item {
            Item::Text { text, rect, .. } if text == "DESIGN.md" => Some(*rect),
            _ => None,
        })
        .expect("DESIGN.md span drawn");
    rx.try_iter().for_each(drop);
    host.pointer_move(Some([
        design[0] + design[2] / 2.,
        design[1] + design[3] / 2.,
    ]));
    frame(&mut host);
    std::thread::sleep(Duration::from_millis(300));
    frame(&mut host);
    let targets: Vec<String> = rx
        .try_iter()
        .filter(|r| r["type"] == "previewCode")
        .filter_map(|r| r["path"].as_str().map(String::from))
        .collect();
    assert!(
        targets.iter().any(|t| t == "DESIGN.md"),
        "previewed {targets:?}"
    );
    assert!(
        !targets.iter().any(|t| t == "README.md"),
        "previewed {targets:?}"
    );
}

/// The local preview loads live. Both the preview card and a public URL open in the tiling.
#[test]
fn url_previews_and_terminal_links_open_in_browser_panes() {
    let (tx, rx) = mpsc::channel();
    let mut next_id = 9;
    let mut host = Host::with_request_handler(
        format!("{APP}\nviewportSize.value = [640, 800];"),
        "{}".into(),
        TextEngine::default(),
        move |raw| {
            let request: Value = serde_json::from_str(&raw).unwrap();
            let response = if request["type"] == "openTile" {
                let id = next_id;
                next_id += 1;
                id.to_string()
            } else {
                "ok".into()
            };
            tx.send(request).unwrap();
            Ok(response)
        },
    )
    .unwrap();
    install_fonts(&mut host);
    let link = |target: &str| json!({"kind": "url", "target": target, "line": null, "col": null, "resolved": false, "image": false});
    let span = |text: &str, col: usize, link: Option<usize>| {
        let mut s = json!({"text": text, "col": col, "cells": text.len(), "fg": 0xffffffffu32, "bold": false, "underline": link.is_some(), "link": link.is_some()});
        if let Some(i) = link {
            s["linkIndex"] = json!(i);
        }
        s
    };
    let (local, remote) = ("http://localhost:5173/", "https://example.com/");
    let plain = format!("{local} or {remote}");
    let screen = json!({"cols": 80, "rowsCount": 2, "cursorRow": 1, "cursorCol": 0, "cursorVisible": false, "rows": [
        {"idx": 0, "plain": plain, "links": [link(local), link(remote)], "spans": [
            span(local, 0, Some(0)), span(" or ", local.len(), None), span(remote, local.len() + 4, Some(1))]},
        {"idx": 1, "plain": "", "links": [], "spans": []}]});
    let mut tiling = Workspaces::new(1, TilingMode::Niri);
    // Both links on one row need the full width (niri opens columns at half).
    tiling.maximize_column();
    let push_layout = |host: &mut Host, tiling: &Workspaces| {
        host.push(json!({"view": "tiling", "data": tiling.snapshot()}).to_string());
    };
    host.push(
        json!({"view": "sessions", "data": {"activeId": 1, "archivedCount": 0, "items": [
            {"id": 1, "title": "dev", "cli": "terminal", "cwd": "/project", "alive": true}
        ]}})
        .to_string(),
    );
    push_layout(&mut host, &tiling);
    host.push(json!({"view": "screens", "data": {"1": screen}}).to_string());
    let drawn = frame(&mut host);
    let rect_of = |frame: &Frame, name: &str| {
        frame.items.iter().find_map(|item| match item {
            Item::Text { text, rect, .. } if text == name => Some(*rect),
            _ => None,
        })
    };
    let hover = |host: &mut Host, rect: [f32; 4]| {
        host.pointer_move(Some([rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.]));
        frame(host);
        std::thread::sleep(Duration::from_millis(300));
        frame(host)
    };
    let browser_ops = |rx: &mpsc::Receiver<Value>| {
        rx.try_iter()
            .filter(|r| r["type"] == "browser")
            .collect::<Vec<_>>()
    };

    // Public URLs show a card without loading the site.
    browser_ops(&rx);
    let public_card = hover(&mut host, rect_of(&drawn, remote).expect("remote link"));
    assert!(rect_of(&public_card, "Click to open in a browser pane").is_some());
    assert!(
        browser_ops(&rx).iter().all(|r| r["op"] != "open"),
        "external URLs must not load on hover"
    );

    // Local: the shared preview page opens at the desktop size and the card shows its frame.
    let card = hover(&mut host, rect_of(&drawn, local).expect("local link"));
    let ops = browser_ops(&rx);
    let open = ops
        .iter()
        .find(|r| r["op"] == "open")
        .expect("preview page opened");
    assert_eq!(open["url"], local);
    assert!(
        open["width"].as_f64().unwrap() > 1000.,
        "renders at a desktop width"
    );
    assert!(rect_of(&card, "Click to open in a browser pane").is_some());
    let id = open["id"].as_u64().unwrap();
    let marker = [4, 5, 6, 255];
    host.set_image(format!("browser:{id}"), marker.to_vec(), [1, 1]);
    for loading in [true, false] {
        host.push(
            json!({"view": "browserState", "data": {"id": id, "url": local, "title": "Vite App",
            "loading": loading, "canBack": false, "canForward": false, "error": null}})
            .to_string(),
        );
    }
    let page = frame(&mut host);
    assert!(
        page.items
            .iter()
            .any(|item| matches!(item, Item::Image { image, .. } if image.rgba.as_ref() == marker))
    );
    assert!(rect_of(&page, "Vite App").is_some());

    // The card stays open when the pointer leaves the link and moves onto the card itself.
    let footer = rect_of(&page, "Click to open in a browser pane").expect("preview footer");
    host.pointer_move(Some([
        footer[0] + footer[2] / 2.,
        footer[1] + footer[3] / 2.,
    ]));
    frame(&mut host);
    std::thread::sleep(Duration::from_millis(240));
    assert!(rect_of(&frame(&mut host), "Vite App").is_some());
    host.pointer_down(jsnir::editor::PointSelection::Place);
    host.pointer_up();
    frame(&mut host);
    let requests: Vec<Value> = rx.try_iter().collect();
    let opened = requests
        .iter()
        .position(|r| r["type"] == "openTile")
        .expect("a pane for the local URL");
    assert!(
        requests[..opened].iter().any(|r| r["op"] == "hidden"),
        "the preview pauses first: {requests:?}"
    );
    assert!(
        requests
            .iter()
            .all(|r| r["type"] != "openExternal" && !(r["type"] == "open" && r["kind"] == "url"))
    );

    tiling.open_pane(9, false);
    push_layout(&mut host, &tiling);
    frame(&mut host);
    std::thread::sleep(Duration::from_millis(60));
    frame(&mut host);
    assert!(
        browser_ops(&rx)
            .iter()
            .any(|r| r["op"] == "open" && r["id"] == 9 && r["url"] == local)
    );

    tiling.focus_session(1);
    tiling.toggle_maximize();
    push_layout(&mut host, &tiling);
    let terminal = frame(&mut host);
    let link = rect_of(&terminal, remote).expect("public terminal link");
    host.pointer_move(Some([link[0] + link[2] / 2., link[1] + link[3] / 2.]));
    frame(&mut host);
    host.pointer_down(jsnir::editor::PointSelection::Place);
    host.pointer_up();
    frame(&mut host);
    let requests: Vec<_> = rx.try_iter().collect();
    assert_eq!(
        requests.iter().filter(|r| r["type"] == "openTile").count(),
        1
    );
    assert!(
        requests
            .iter()
            .all(|r| r["type"] != "openExternal" && !(r["type"] == "open" && r["kind"] == "url"))
    );
    tiling.open_pane(10, false);
    push_layout(&mut host, &tiling);
    frame(&mut host);
    std::thread::sleep(Duration::from_millis(60));
    frame(&mut host);
    assert!(
        browser_ops(&rx)
            .iter()
            .any(|r| r["op"] == "open" && r["id"] == 10 && r["url"] == remote)
    );
}

/// A Git pane has one header: workspace-style tabs (outline glyph) instead of a "Git" title plus a
/// second tab row, and it stays even when the pane is fullscreen.
#[test]
fn git_pane_has_a_single_header_with_workspace_style_tabs() {
    let (tx, rx) = mpsc::channel();
    let mut host = Host::with_request_handler(
        format!(
            r#"{APP}
        tileSpecs.value = {{5: {{kind: 'review', path: '', cwd: '/p'}}}};
        mount(() => TilePane({{id: () => 5, focused: () => true, multiPane: () => false}}));"#
        ),
        "{}".into(),
        TextEngine::default(),
        move |raw| {
            tx.send(serde_json::from_str::<Value>(&raw).unwrap())
                .unwrap();
            Ok("ok".into())
        },
    )
    .unwrap();
    install_fonts(&mut host);
    let marker = [7, 7, 7, 255];
    host.set_image(
        "glyph:workspace-tab-outline".into(),
        marker.to_vec(),
        [1, 1],
    );
    let drawn = frame(&mut host);
    let texts: Vec<&str> = drawn
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts.iter().filter(|t| **t == "Changes").count(),
        1,
        "{texts:?}"
    );
    assert!(texts.contains(&"Graph"));
    assert!(!texts.contains(&"Git"), "no separate title: {texts:?}");
    assert!(
        drawn
            .items
            .iter()
            .any(|item| matches!(item, Item::Image { image, .. } if image.rgba.as_ref() == marker))
    );
    let graph = drawn
        .items
        .iter()
        .find_map(|item| match item {
            Item::Text { text, rect, .. } if text == "Graph" => Some(*rect),
            _ => None,
        })
        .unwrap();
    rx.try_iter().for_each(drop);
    host.pointer_move(Some([graph[0] + 2., graph[1] + graph[3] / 2.]));
    host.pointer_down(jsnir::editor::PointSelection::Place);
    host.pointer_up();
    frame(&mut host);
    assert!(
        rx.try_iter().any(|r| r["type"] == "gitGraph"),
        "the Graph tab loads the log"
    );
}

/// The sidebar's working spinner steps through its rotated frames while the agent works.
#[test]
fn working_spinner_animates_through_frames() {
    let mut host = Host::with_request_handler(format!(r#"{APP}
        sessionsState.value = {{activeId: 1, items: [{{id: 1, title: 'Agent', cwd: '/p', cli: 'claude', alive: true, status: 'working'}}]}};
        mount(PriorityAgents);"#),
        "{}".into(), TextEngine::default(), |_| Ok("ok".into())).unwrap();
    install_fonts(&mut host);
    for frame_index in 0..12u8 {
        host.set_image(
            format!("glyph:agent-working-{frame_index}"),
            vec![frame_index, 1, 2, 255],
            [1, 1],
        );
    }
    let shown = |host: &mut Host| {
        host.poll();
        host.frame(640., 800., 1., 4096)
            .items
            .iter()
            .find_map(|item| match item {
                Item::Image { image, .. } if image.rgba[1] == 1 && image.rgba[2] == 2 => {
                    Some(image.rgba[0])
                }
                _ => None,
            })
    };
    let mut seen = std::collections::BTreeSet::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(600) {
        if let Some(index) = shown(&mut host) {
            seen.insert(index);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(seen.len() >= 3, "spinner frames shown: {seen:?}");
}

/// OSC 8 hyperlinks and soft-wrapped URLs/paths stay intact and resolve across screen rows.
#[test]
fn osc8_and_wrapped_terminal_links_resolve_on_every_row() {
    use icy_core::vt::{AlacrittyVt, VtEngine};

    let dir = std::env::temp_dir().join(format!("icy-links-wrap-{}", std::process::id()));
    let nested = dir.join("deep/folder/name");
    std::fs::create_dir_all(&nested).unwrap();
    let img = nested.join("preview_shot.png");
    std::fs::write(&img, b"fake").unwrap();

    // 1. Narrow terminal forces the path and URL to soft-wrap across two rows.
    let mut vt = AlacrittyVt::new(28, 6);
    let url = "https://icons.getbootstrap.com/icons/activity/";
    vt.feed(format!("deep/folder/name/preview_shot.png\r\n{url}\r\n").as_bytes());
    // 2. OSC 8 hyperlink with short display label pointing to the real image file.
    let uri = format!("file:///{}", img.to_string_lossy().trim_start_matches('/'));
    vt.feed(format!("see \x1b]8;;{uri}\x07shot\x1b]8;;\x07\r\n").as_bytes());

    let rows = terminal_rows_payload(&vt.snapshot().rows, &dir);
    // Wrapped image path resolves on both row 0 and row 1.
    for (r, row) in rows[0..=1].iter().enumerate() {
        assert_eq!(row.links.len(), 1, "row {r}: {:?}", row.links);
        assert_eq!(row.links[0].target, "deep/folder/name/preview_shot.png");
        assert!(row.links[0].resolved, "row {r} should resolve");
        assert!(row.links[0].image, "row {r} should be an image tag");
    }
    // Wrapped URL stays intact on both row 2 and row 3.
    for (offset, row) in rows[2..=3].iter().enumerate() {
        let r = offset + 2;
        assert_eq!(row.links.len(), 1, "row {r}: {:?}", row.links);
        assert_eq!(row.links[0].target, url);
        assert_eq!(row.links[0].kind, RefKind::Url);
    }
    // OSC 8 hyperlink on row 4 resolves to the image file despite short label "shot".
    assert_eq!(rows[4].links.len(), 1, "row 4: {:?}", rows[4].links);
    assert!(rows[4].links[0].resolved);
    assert!(rows[4].links[0].image);
    assert!(
        rows[4]
            .spans
            .iter()
            .any(|s| s.text == "shot" && s.image && s.link_index == Some(0))
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn terminal_link_offsets_count_cells_inside_a_mixed_unicode_run() {
    use icy_core::vt::{AlacrittyVt, VtEngine};

    let mut vt = AlacrittyVt::new(80, 2);
    vt.feed("└ café https://example.com ░ fin".as_bytes());
    let screen = vt.snapshot();
    assert_eq!(screen.rows[0].runs.len(), 1);
    let rows = terminal_rows_payload(&screen.rows, Path::new("."));
    let spans: Vec<_> = rows[0]
        .spans
        .iter()
        .map(|span| (span.text.as_str(), span.col, span.cells, span.link_index))
        .collect();
    assert_eq!(
        spans,
        [
            ("└ café ", 0, 7, None),
            ("https://example.com", 7, 19, Some(0)),
            (" ░ fin", 26, 6, None),
        ]
    );
}

/// Drag-selecting inside a paint-offset pane on a word-wrapped line hits the exact characters.
#[test]
fn source_editor_wrapped_and_offset_selection_hits_exact_columns() {
    let line =
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="176" height="38" viewBox="0 0 176 38">"#;
    let script = format!(
        r#"{APP}
        const spec = {{ kind: 'file', path: 'icon.svg', cwd: '/project' }};
        const fixture = {{ path: '/project/icon.svg', displayPath: 'icon.svg', editableText: {line:?},
          lines: [{{ num: 1, text: {line:?} }}] }};
        // Simulate a paint-offset tiled column (paint.x = 60, width = 576 -> wraps before `viewBox`).
        mount(() => Column({{
          style: {{ width: 640, height: 800 }},
          children: [Box({{
            style: {{ position: 'absolute', left: 0, top: 0, width: 576, height: 300 }},
            paint: {{ x: 60, y: 40 }},
            children: [SourceEditor({{ spec, doc: () => fixture, paintOffset: () => [60, 40] }})]
          }})]
        }}));
        "#
    );
    let mut host = Host::with_request_handler(script, "{}".into(), TextEngine::default(), |_| {
        Ok("ok".into())
    })
    .unwrap();
    install_fonts(&mut host);
    let initial = frame(&mut host);
    // Find the code line text box (starts to the right of the 48px gutter + 14px gap, plus paint.x = 60).
    let code_rect = initial
        .items
        .iter()
        .find_map(|item| match item {
            Item::Text { text, rect, .. } if text.starts_with("<svg") => Some(*rect),
            _ => None,
        })
        .expect("wrapped svg line");
    // Visual line 2 starts with `viewBox` (1 line below code_rect[1]).
    let y2 = code_rect[1] + 12.5 * 1.5 + 4.0;
    let start_x = code_rect[0] + 1.0;
    let end_x = code_rect[0] + 7.0 * (12.5 * 0.6); // 7 chars = "viewBox"
    host.pointer_move(Some([start_x, y2]));
    host.pointer_down(jsnir::editor::PointSelection::Place);
    host.pointer_move(Some([end_x, y2]));
    host.pointer_up();
    let after = frame(&mut host);
    // `viewBox` begins at index 64 (Col 65) and is 7 characters long -> ends at Col 72 (7 selected).
    let footer = after
        .items
        .iter()
        .find_map(|item| match item {
            Item::Text { text, .. } if text.starts_with("Ln 1, Col ") => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_else(|| {
            let texts: Vec<_> = after
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            panic!("missing caret footer (code_rect={code_rect:?}): {texts:?}")
        });
    assert_eq!(footer, "Ln 1, Col 72 (7 selected)");
}

#[test]
fn workspace_frame_rounds_all_four_corners_and_draws_active_tab_above_top_stroke() {
    const OUTLINE: [u8; 4] = [33, 44, 55, 255];
    let script = format!(
        r#"{APP}
        viewportSize.value = [640, 800];
        sessionsState.value = {{activeId: 1, items: [{{id: 1, title: 'Shell', cli: 'terminal', cwd: '/project', alive: true}}]}};
        tilingState.value = {{
            mode: 'niri', focusedSession: 1, maximized: false, viewOffset: 0, stripWidth: 1, totalCols: 1,
            columns: [{{id: 1, panes: [1], heights: [1], x: 0, width: 1, expanded: false, visible: true}}],
            workspaces: [{{panes: [1], focused: 1, columns: [], viewOffset: 0}}],
            activeWorkspace: 0,
        }};
        mount(WorkspaceShell);
    "#
    );
    let mut host = Host::with_request_handler(script, "{}".into(), TextEngine::default(), |_| {
        Ok("ok".into())
    })
    .unwrap();
    host.set_reduced_motion(true);
    install_fonts(&mut host);
    host.set_image(
        "glyph:workspace-tab-outline".into(),
        OUTLINE.to_vec(),
        [1, 1],
    );
    host.set_image(
        "glyph:workspace-tab-fill".into(),
        [1, 2, 3, 255].to_vec(),
        [1, 1],
    );
    let drawn = frame(&mut host);
    let (frame_border_idx, frame_rect) = drawn
        .items
        .iter()
        .enumerate()
        .find_map(|(i, item)| match item {
            Item::Inset {
                spread,
                radius,
                rect,
                ..
            } if (*spread - 1.0).abs() < 0.01
                && *radius == [8.0, 8.0, 8.0, 8.0]
                && rect[2] > 300.0
                && rect[3] > 500.0 =>
            {
                Some((i, *rect))
            }
            _ => None,
        })
        .expect("workspace frame must draw a 1px border with all four 8px rounded corners");
    let (tab_outline_idx, tab_rect) = drawn
        .items
        .iter()
        .enumerate()
        .find_map(|(i, item)| match item {
            Item::Image { image, rect, .. } if image.rgba.as_ref() == OUTLINE => Some((i, *rect)),
            _ => None,
        })
        .expect("active tab outline");
    assert!(
        tab_outline_idx > frame_border_idx,
        "active tab outline ({tab_outline_idx}) must paint above the workspace frame border ({frame_border_idx})"
    );
    assert!(
        ((tab_rect[1] + tab_rect[3] - 1.0) - frame_rect[1]).abs() < 0.01,
        "active tab bottom flare must align with the frame's top 1px stroke: tab={tab_rect:?} frame={frame_rect:?}"
    );
}

#[test]
fn browser_pane_insets_its_page_inside_the_window_border_with_rounded_clip() {
    const PAGE: [u8; 4] = [19, 29, 39, 255];
    let script = format!(
        r#"{APP}
        tileSpecs.value = {{1: {{kind: 'browser', path: '', url: 'https://example.com'}}}};
        mount(() => TilePane({{id: () => 1, focused: () => true, multiPane: () => true}}));
    "#
    );
    let mut host = Host::with_request_handler(script, "{}".into(), TextEngine::default(), |_| {
        Ok("ok".into())
    })
    .unwrap();
    host.set_reduced_motion(true);
    install_fonts(&mut host);
    host.set_image("browser:1".into(), PAGE.to_vec(), [1, 1]);
    let drawn = frame(&mut host);
    let border = drawn
        .items
        .iter()
        .find_map(|item| match item {
            Item::Inset {
                rect,
                spread,
                radius,
                ..
            } if (*spread - 0.5).abs() < 0.01
                && *radius == [10.0, 10.0, 10.0, 10.0]
                && rect[2] > 500.0 =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("outer hairline Niri window border with 10px corners");
    let (page_rect, round_clip) = drawn
        .items
        .iter()
        .find_map(|item| match item {
            Item::Image {
                image,
                rect,
                round_clip,
                ..
            } if image.rgba.as_ref() == PAGE => Some((*rect, *round_clip)),
            _ => None,
        })
        .expect("browser page image");
    assert!(
        page_rect[0] >= border[0] + 1.0 - 0.01,
        "left border preserved: page={page_rect:?} border={border:?}"
    );
    assert!(
        page_rect[0] + page_rect[2] <= border[0] + border[2] - 1.0 + 0.01,
        "right border preserved: page={page_rect:?} border={border:?}"
    );
    assert!(
        page_rect[1] + page_rect[3] <= border[1] + border[3] - 1.0 + 0.01,
        "bottom border preserved: page={page_rect:?} border={border:?}"
    );
    let (clip_rect, clip_radius) =
        round_clip.expect("browser page clipped to inner window corners");
    assert_eq!(clip_radius, [9.0, 9.0, 9.0, 9.0]);
    assert!((clip_rect[0] - (border[0] + 1.0)).abs() < 0.01);
    assert!((clip_rect[1] - (border[1] + 1.0)).abs() < 0.01);
}

/// Frame cost at a 1440p window: `cargo test --release -p icy-app frame_budget -- --ignored --nocapture`.
#[test]
#[ignore]
fn frame_budget() {
    let mut host =
        Host::with_request_handler(APP.into(), "{}".into(), TextEngine::default(), |raw| {
            let request: Value = serde_json::from_str(&raw).unwrap();
            Ok(match request["type"].as_str() {
                Some("browseProject") => {
                    json!({"path": "/p", "parent": "/", "ancestors": [], "folders": []}).to_string()
                }
                _ => "ok".into(),
            })
        })
        .unwrap();
    install_fonts(&mut host);
    install_images_on_host(&mut host, 2.);
    let measure = |host: &mut Host, label: &str| {
        let mut times = Vec::new();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            host.poll();
            let t = Instant::now();
            let _ = host.frame(2464., 1493., 2., 8192);
            times.push(t.elapsed().as_secs_f64() * 1e3);
            std::thread::sleep(Duration::from_millis(6));
        }
        times.sort_by(f64::total_cmp);
        let p = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
        println!(
            "{label}: n={} p50={:.2}ms p90={:.2}ms p99={:.2}ms max={:.2}ms",
            times.len(),
            p(0.5),
            p(0.9),
            p(0.99),
            p(1.0)
        );
    };
    host.push(json!({"view": "workspace", "data": "/p"}).to_string());
    host.push(
        json!({"view": "tiling", "data": Workspaces::new(1, TilingMode::Niri).snapshot()})
            .to_string(),
    );
    measure(&mut host, "empty workspace");
    let mut tiling = Workspaces::new(1, TilingMode::Niri);
    tiling.open_pane(2, false);
    let items = json!([
        { "id": 1, "title": "a", "cli": "claude", "cwd": "/p", "alive": true },
        { "id": 2, "title": "b", "cli": "codex", "cwd": "/p", "alive": true }
    ]);
    host.push(
        json!({"view": "sessions", "data": {"activeId": 1, "archivedCount": 0, "items": items}})
            .to_string(),
    );
    host.push(json!({"view": "tiling", "data": tiling.snapshot()}).to_string());
    let rows: Vec<Value> = (0..70)
        .map(|i| json!({"idx": i, "plain": format!("{i:>3} fn example_{i}() -> Result<(), Error> {{ let value = compute(42) * {i}; Ok(()) }}"), "spans": [], "links": []}))
        .collect();
    let screen = json!({"cols": 160, "rowsCount": 70, "cursorRow": 0, "cursorCol": 0, "cursorVisible": true, "rows": rows});
    host.push(json!({"view": "screens", "data": {"1": screen, "2": screen}}).to_string());
    measure(&mut host, "two terminals (idle)");
    let mut tick = 0;
    let mut times = Vec::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        tick += 1;
        // An agent streaming output: one changed row per frame.
        let mut rows = rows.clone();
        rows[tick % 70] = json!({"idx": tick % 70, "plain": format!("streaming {tick}"), "spans": [], "links": []});
        let screen = json!({"cols": 160, "rowsCount": 70, "cursorRow": 0, "cursorCol": 0, "cursorVisible": true, "rows": rows});
        host.push(json!({"view": "screens", "data": {"1": screen}}).to_string());
        host.poll();
        let t = Instant::now();
        let _ = host.frame(2464., 1493., 2., 8192);
        times.push(t.elapsed().as_secs_f64() * 1e3);
        std::thread::sleep(Duration::from_millis(6));
    }
    times.sort_by(f64::total_cmp);
    println!(
        "streaming terminal: n={} p50={:.2}ms p90={:.2}ms max={:.2}ms",
        times.len(),
        times[times.len() / 2],
        times[times.len() * 9 / 10],
        times[times.len() - 1]
    );
}
