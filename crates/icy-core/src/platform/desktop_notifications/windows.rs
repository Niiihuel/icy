use std::{mem::ManuallyDrop, os::windows::ffi::OsStringExt, path::PathBuf};

use ::windows::{
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{NotificationSetting, ToastNotification, ToastNotificationManager},
    Win32::{
        Storage::EnhancedStorage::PKEY_AppUserModel_ID,
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, IPersistFile,
                StructuredStorage::{
                    PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
                    PropVariantClear,
                },
            },
            Variant::VT_LPWSTR,
            WinRT::{RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize},
        },
        UI::Shell::{
            FOLDERID_Programs, IShellLinkW, KF_FLAG_CREATE, PropertiesSystem::IPropertyStore,
            SHGetKnownFolderPath, SHStrDupW, ShellLink,
        },
    },
    core::{HSTRING, Interface, h, w},
};

use super::DesktopNotification;

const APP_ID: &str = "Niiihuel.icy";

#[derive(Default)]
pub(super) struct Notifier {
    registered: bool,
}

struct Apartment;

impl Apartment {
    fn enter() -> ::windows::core::Result<Self> {
        // SAFETY: The notification worker owns this thread and balances initialization in Drop.
        unsafe { RoInitialize(RO_INIT_SINGLETHREADED)? };
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

impl Notifier {
    pub(super) fn show(
        &mut self,
        notice: &DesktopNotification,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _apartment = Apartment::enter()?;
        let app_id = HSTRING::from(APP_ID);
        if !self.registered {
            register_shortcut(&app_id)?;
            self.registered = true;
        }
        let xml = XmlDocument::new()?;
        xml.LoadXml(h!("<toast><visual><binding template=\"ToastGeneric\"><text/><text/></binding></visual></toast>"))?;
        let fields = xml.GetElementsByTagName(h!("text"))?;
        for (index, line) in [&notice.title, &notice.body].into_iter().enumerate() {
            fields
                .Item(index as u32)?
                .AppendChild(&xml.CreateTextNode(&HSTRING::from(line))?)?;
        }
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&app_id)?;
        if notifier.Setting()? != NotificationSetting::Enabled {
            return Err(std::io::Error::other("Windows has disabled notifications for icy").into());
        }
        notifier.Show(&ToastNotification::CreateToastNotification(&xml)?)?;
        Ok(())
    }
}

/// A portable Win32 binary also needs a Start Menu link with its app ID for Windows to route toasts.
fn register_shortcut(app_id: &HSTRING) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let executable = std::env::current_exe()?;
    // SAFETY: COM is initialized on this thread. Both Windows-allocated strings are freed below.
    unsafe {
        let programs = SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_CREATE, None)?;
        let path = PathBuf::from(std::ffi::OsString::from_wide(programs.as_wide()));
        CoTaskMemFree(Some(programs.as_ptr().cast()));
        let directory = path.join("icy");
        std::fs::create_dir_all(&directory)?;
        let shortcut = HSTRING::from(directory.join("icy.lnk").as_os_str());
        let target = HSTRING::from(executable.as_os_str());
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&target)?;
        link.SetDescription(w!("icy terminal workspace"))?;
        let store: IPropertyStore = link.cast()?;
        let mut value = PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_LPWSTR,
                    Anonymous: PROPVARIANT_0_0_0 {
                        pwszVal: SHStrDupW(app_id)?,
                    },
                    ..Default::default()
                }),
            },
        };
        let stored = store.SetValue(&PKEY_AppUserModel_ID, &value);
        PropVariantClear(&mut value)?;
        stored?;
        store.Commit()?;
        let persist: IPersistFile = link.cast()?;
        persist.Save(&shortcut, true)?;
    }
    Ok(())
}
