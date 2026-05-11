use anyhow::{Context, Result};
use windows::core::{IUnknown, IUnknown_Vtbl, Interface, GUID};
use windows::Win32::Foundation::{BOOL, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IServiceProvider, CLSCTX_LOCAL_SERVER,
    COINIT_APARTMENTTHREADED,
};

const CLSID_IMMERSIVE_SHELL: GUID = GUID::from_u128(0xc2f03a33_21f5_47fa_b4bb_156362a2f239);
const SID_IMMERSIVE_SETTINGS_CACHE: GUID = GUID::from_u128(0x53660488_8855_460b_a9ab_5cfc6b5012ca);

const TOUCHPAD_SETTING_IDS: &[u32] = &[
    0x0F, // ThreeFingerTapEnabled
    0x10, // FourFingerTapEnabled
    0x11, // ThreeFingerSlideEnabled
    0x12, // FourFingerSlideEnabled
    0x13, // MultiTaskingAltTabFilter
    0x14, 0x15, // ThreeFingerLeft + key params
    0x16, 0x17, // ThreeFingerUp + key params
    0x18, 0x19, // ThreeFingerRight + key params
    0x1A, 0x1B, // ThreeFingerDown + key params
    0x1C, 0x1D, // FourFingerLeft + key params
    0x1E, 0x1F, // FourFingerRight + key params
    0x20, 0x21, // FourFingerUp + key params
    0x22, 0x23, // FourFingerDown + key params
    0x24, 0x25, // CustomThreeFingerTap + key params
    0x26, 0x27, // CustomFourFingerTap + key params
];

pub(super) fn apply_touchpad_setting_changes() -> Result<()> {
    let _com = ComApartment::initialize()?;

    let provider: IServiceProvider =
        unsafe { CoCreateInstance(&CLSID_IMMERSIVE_SHELL, None, CLSCTX_LOCAL_SERVER) }
            .context("Creating Immersive Shell COM provider")?;
    let cache: IImmersiveSettingsCache =
        unsafe { provider.QueryService(&SID_IMMERSIVE_SETTINGS_CACHE) }
            .context("Opening Immersive Settings cache")?;

    for id in TOUCHPAD_SETTING_IDS {
        unsafe {
            cache
                .on_setting_changed(*id)
                .with_context(|| format!("Invalidating touchpad settings cache id {id:#x}"))?;
        }
    }

    Ok(())
}

struct ComApartment {
    uninitialize: bool,
}

impl ComApartment {
    fn initialize() -> Result<Self> {
        match unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) } {
            Ok(()) => Ok(Self { uninitialize: true }),
            Err(err) if err.code() == RPC_E_CHANGED_MODE => Ok(Self {
                uninitialize: false,
            }),
            Err(err) => Err(err).context("Initializing COM for touchpad settings cache"),
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct IImmersiveSettingsCache(IUnknown);

impl IImmersiveSettingsCache {
    unsafe fn on_setting_changed(&self, setting_id: u32) -> windows::core::Result<()> {
        (Interface::vtable(self).OnSettingChanged)(Interface::as_raw(self), setting_id).ok()
    }
}

impl windows::core::CanInto<IUnknown> for IImmersiveSettingsCache {}

unsafe impl Interface for IImmersiveSettingsCache {
    type Vtable = IImmersiveSettingsCache_Vtbl;
}

unsafe impl windows::core::ComInterface for IImmersiveSettingsCache {
    const IID: GUID = GUID::from_u128(0x4214f6fa_eb36_4e2f_9ca2_23fdc1832df7);
}

#[repr(C)]
#[allow(non_snake_case)]
struct IImmersiveSettingsCache_Vtbl {
    base__: IUnknown_Vtbl,
    OnSettingChanged: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        setting_id: u32,
    ) -> windows::core::HRESULT,
    GetBOOL: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        setting_id: u32,
        value: *mut BOOL,
    ) -> windows::core::HRESULT,
    GetDWORD: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        setting_id: u32,
        value: *mut u32,
    ) -> windows::core::HRESULT,
    RegisterForSettingChange: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        notification: *mut core::ffi::c_void,
        token: *mut u32,
    ) -> windows::core::HRESULT,
    UnregisterForSettingChange: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        token: u32,
    ) -> windows::core::HRESULT,
}
