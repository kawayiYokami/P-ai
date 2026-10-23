// ==================== Windows 平台实现（由 windows_tool.rs + ui_automation.rs 平移，不重写） ====================
//
// 仅 Windows 编译（platform/mod.rs 按 cfg 引入）。包含：
// - list windows：EnumWindows 全量枚举（绕 xcap 同进程过滤，含 PAI 自身窗口）
// - activate window：SW_RESTORE + Alt 键技巧 + SetForegroundWindow + 轮询验证
// - 控件树：手写 UIA COM vtable 绑定（不引入 windows crate，仅 windows-core + windows-sys）

use super::{AppClickOutcome, AppTarget, MAX_ELEMENTS, UiElementInfo, WindowInfo};
use crate::{runtime_log_info, runtime_log_warn};

// ==================== UI Automation 绑定 ====================
// 接口定义复制自 windows-0.61.3 生成代码（windows_core::imp::define_interface! 宏）。
//
// 关键约束：IUIAutomation 的 vtable 槽位必须全量声明，即使本项目用不到。
// 原因是 IUIAutomation2 等派生接口的 vtable 直接内嵌基础 vtable（base__: IUIAutomation_Vtbl），
// 少写一个占位就会让派生接口的方法指到错误的函数上——不会报错，而是静默调用别的函数。
// 新增派生接口时，一律按 SDK 声明顺序在末尾追加，未用到的槽位保留 usize 占位。
// UIA_CONTROLTYPE_ID / UIA_*ControlTypeId 沿用 Windows SDK 官方命名，便于对照微软文档。

use windows_core::imp::define_interface;
use windows_core::imp::interface_hierarchy;
use windows_core::Interface;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct UIA_CONTROLTYPE_ID(pub i32);

// 可交互控件类型白名单（UI Automation ControlType 常量，沿用 SDK 官方命名）
#[allow(non_upper_case_globals)]
pub const UIA_ButtonControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50000);
#[allow(non_upper_case_globals)]
pub const UIA_CheckBoxControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50002);
#[allow(non_upper_case_globals)]
pub const UIA_ComboBoxControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50003);
#[allow(non_upper_case_globals)]
pub const UIA_EditControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50004);
#[allow(non_upper_case_globals)]
pub const UIA_HyperlinkControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50005);
#[allow(non_upper_case_globals)]
pub const UIA_ListItemControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50007);
#[allow(non_upper_case_globals)]
pub const UIA_MenuItemControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50011);
#[allow(non_upper_case_globals)]
pub const UIA_RadioButtonControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50013);
#[allow(non_upper_case_globals)]
pub const UIA_SliderControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50015);
#[allow(non_upper_case_globals)]
pub const UIA_TabItemControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50019);
#[allow(non_upper_case_globals)]
pub const UIA_TreeItemControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50024);
/// Document：记事本/Word 等应用的文本编辑区用这个类型，不在白名单里就永远摸不到编辑区
#[allow(non_upper_case_globals)]
pub const UIA_DocumentControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50030);

// 以下类型不进可交互白名单，仅供 control_type_name 命名映射（避免快照里显示 Unknown 误导模型）
#[allow(non_upper_case_globals)]
pub const UIA_ListControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50008);
#[allow(non_upper_case_globals)]
pub const UIA_MenuBarControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50010);
#[allow(non_upper_case_globals)]
pub const UIA_TabControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50018);
#[allow(non_upper_case_globals)]
pub const UIA_TextControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50020);
#[allow(non_upper_case_globals)]
pub const UIA_CustomControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50025);
#[allow(non_upper_case_globals)]
pub const UIA_WindowControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50032);
#[allow(non_upper_case_globals)]
pub const UIA_PaneControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50033);
#[allow(non_upper_case_globals)]
pub const UIA_StatusBarControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50017);
#[allow(non_upper_case_globals)]
pub const UIA_TitleBarControlTypeId: UIA_CONTROLTYPE_ID = UIA_CONTROLTYPE_ID(50037);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct TreeScope(pub i32);

#[allow(non_upper_case_globals)]
pub const TreeScope_Descendants: TreeScope = TreeScope(4);

/// 可交互控件类型白名单判定（纯函数，便于单测）
#[allow(non_upper_case_globals)]
pub fn is_interactive_control_type(ct: UIA_CONTROLTYPE_ID) -> bool {
    matches!(
        ct,
        UIA_ButtonControlTypeId
            | UIA_CheckBoxControlTypeId
            | UIA_ComboBoxControlTypeId
            | UIA_EditControlTypeId
            | UIA_HyperlinkControlTypeId
            | UIA_ListItemControlTypeId
            | UIA_MenuItemControlTypeId
            | UIA_RadioButtonControlTypeId
            | UIA_SliderControlTypeId
            | UIA_TabItemControlTypeId
            | UIA_TreeItemControlTypeId
            | UIA_DocumentControlTypeId
    )
}

/// UIA 返回的控件类型数值转可读名称（用于模型理解）
#[allow(non_upper_case_globals)]
pub fn control_type_name(ct: UIA_CONTROLTYPE_ID) -> &'static str {
    match ct {
        UIA_ButtonControlTypeId => "Button",
        UIA_CheckBoxControlTypeId => "CheckBox",
        UIA_ComboBoxControlTypeId => "ComboBox",
        UIA_EditControlTypeId => "Edit",
        UIA_HyperlinkControlTypeId => "Hyperlink",
        UIA_ListItemControlTypeId => "ListItem",
        UIA_MenuItemControlTypeId => "MenuItem",
        UIA_RadioButtonControlTypeId => "RadioButton",
        UIA_SliderControlTypeId => "Slider",
        UIA_TabItemControlTypeId => "TabItem",
        UIA_TreeItemControlTypeId => "TreeItem",
        UIA_DocumentControlTypeId => "Document",
        UIA_ListControlTypeId => "List",
        UIA_MenuBarControlTypeId => "MenuBar",
        UIA_TabControlTypeId => "Tab",
        UIA_TextControlTypeId => "Text",
        UIA_CustomControlTypeId => "Custom",
        UIA_WindowControlTypeId => "Window",
        UIA_PaneControlTypeId => "Pane",
        UIA_StatusBarControlTypeId => "StatusBar",
        UIA_TitleBarControlTypeId => "TitleBar",
        _ => "Unknown",
    }
}

// ==================== IUIAutomation / IUIAutomation2 ====================
//
// 槽位顺序必须与 Windows SDK 声明一致：IUIAutomation2 的 vtable 接在 IUIAutomation 全部方法之后，
// 少写一个占位就会让 SetConnectionTimeout 指到别的函数上。未用到的槽位统一 usize 占位。

define_interface!(IUIAutomation, IUIAutomation_Vtbl, 0x30cbe57d_d9d0_452a_ab13_7ac5ac4825ee);
interface_hierarchy!(IUIAutomation, windows_core::IUnknown);

#[allow(non_snake_case)]
#[repr(C)]
pub struct IUIAutomation_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    _compare_elements: usize,
    _compare_runtime_ids: usize,
    _get_root_element: usize,
    pub ElementFromHandle: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    _element_from_point: usize,
    _get_focused_element: usize,
    _get_root_element_build_cache: usize,
    _element_from_handle_build_cache: usize,
    _element_from_point_build_cache: usize,
    _get_focused_element_build_cache: usize,
    _create_tree_walker: usize,
    _control_view_walker: usize,
    _content_view_walker: usize,
    _raw_view_walker: usize,
    _raw_view_condition: usize,
    _control_view_condition: usize,
    _content_view_condition: usize,
    _create_cache_request: usize,
    pub CreateTrueCondition: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    _create_false_condition: usize,
    _create_property_condition: usize,
    _create_property_condition_ex: usize,
    _create_and_condition: usize,
    _create_and_condition_from_array: usize,
    _create_and_condition_from_native_array: usize,
    _create_or_condition: usize,
    _create_or_condition_from_array: usize,
    _create_or_condition_from_native_array: usize,
    _create_not_condition: usize,
    _add_automation_event_handler: usize,
    _remove_automation_event_handler: usize,
    _add_property_changed_event_handler_native_array: usize,
    _add_property_changed_event_handler: usize,
    _remove_property_changed_event_handler: usize,
    _add_structure_changed_event_handler: usize,
    _remove_structure_changed_event_handler: usize,
    _add_focus_changed_event_handler: usize,
    _remove_focus_changed_event_handler: usize,
    _remove_all_event_handlers: usize,
    _int_native_array_to_safe_array: usize,
    _int_safe_array_to_native_array: usize,
    _rect_to_variant: usize,
    _variant_to_rect: usize,
    _safe_array_to_rect_native_array: usize,
    _create_proxy_factory_entry: usize,
    _proxy_factory_mapping: usize,
    _get_property_programmatic_name: usize,
    _get_pattern_programmatic_name: usize,
    _poll_for_potential_supported_patterns: usize,
    _poll_for_potential_supported_properties: usize,
    _check_not_supported: usize,
    _reserved_not_supported_value: usize,
    _reserved_mixed_attribute_value: usize,
    _element_from_iaccessible: usize,
    _element_from_iaccessible_build_cache: usize,
}

// IUIAutomation2：只为拿协议层超时能力。v1 的 CUIAutomation 没有超时，
// 目标窗口的 UI 线程一旦停止泵消息，调用就会无限阻塞；v2 的 ConnectionTimeout/TransactionTimeout
// 让这类调用以 ERROR_TIMEOUT 返回，把「挂死」变成「可上报的失败」。
define_interface!(IUIAutomation2, IUIAutomation2_Vtbl, 0x34723aff_0c9d_49d0_9896_7ab52df8cd8a);
interface_hierarchy!(IUIAutomation2, windows_core::IUnknown, IUIAutomation);

impl core::ops::Deref for IUIAutomation2 {
    type Target = IUIAutomation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}

#[allow(non_snake_case)]
#[repr(C)]
pub struct IUIAutomation2_Vtbl {
    pub base__: IUIAutomation_Vtbl,
    _auto_set_focus: usize,
    _set_auto_set_focus: usize,
    pub ConnectionTimeout: unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub SetConnectionTimeout: unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub TransactionTimeout: unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub SetTransactionTimeout: unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
}

#[allow(non_snake_case)]
impl IUIAutomation2 {
    pub unsafe fn ConnectionTimeout(&self) -> windows_core::Result<u32> {
        let mut result = 0u32;
        let hr = (windows_core::Interface::vtable(self).ConnectionTimeout)(windows_core::Interface::as_raw(self), &mut result);
        if hr.is_ok() { Ok(result) } else { Err(hr.into()) }
    }

    pub unsafe fn SetConnectionTimeout(&self, timeout: u32) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).SetConnectionTimeout)(windows_core::Interface::as_raw(self), timeout).ok()
    }

    pub unsafe fn TransactionTimeout(&self) -> windows_core::Result<u32> {
        let mut result = 0u32;
        let hr = (windows_core::Interface::vtable(self).TransactionTimeout)(windows_core::Interface::as_raw(self), &mut result);
        if hr.is_ok() { Ok(result) } else { Err(hr.into()) }
    }

    pub unsafe fn SetTransactionTimeout(&self, timeout: u32) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).SetTransactionTimeout)(windows_core::Interface::as_raw(self), timeout).ok()
    }
}

#[allow(non_snake_case)]
impl IUIAutomation {
    pub unsafe fn ElementFromHandle(&self, hwnd: *mut core::ffi::c_void) -> windows_core::Result<IUIAutomationElement> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).ElementFromHandle)(windows_core::Interface::as_raw(self), hwnd, &mut result__)
            .and_then(|| windows_core::Type::from_abi(result__))
    }

    pub unsafe fn CreateTrueCondition(&self) -> windows_core::Result<IUIAutomationCondition> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).CreateTrueCondition)(windows_core::Interface::as_raw(self), &mut result__)
            .and_then(|| windows_core::Type::from_abi(result__))
    }
}

// ==================== IUIAutomationElement ====================

define_interface!(IUIAutomationElement, IUIAutomationElement_Vtbl, 0xd22108aa_8ac5_49a5_837b_37bbb3d7591e);
interface_hierarchy!(IUIAutomationElement, windows_core::IUnknown);

#[allow(non_snake_case)]
#[repr(C)]
pub struct IUIAutomationElement_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    _set_focus: usize,
    _get_runtime_id: usize,
    _find_first: usize,
    pub FindAll: unsafe extern "system" fn(*mut core::ffi::c_void, TreeScope, *mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    _find_first_build_cache: usize,
    _find_all_build_cache: usize,
    _build_updated_cache: usize,
    _get_current_property_value: usize,
    _get_current_property_value_ex: usize,
    _get_cached_property_value: usize,
    _get_cached_property_value_ex: usize,
    _get_current_pattern_as: usize,
    _get_cached_pattern_as: usize,
    pub GetCurrentPattern: unsafe extern "system" fn(*mut core::ffi::c_void, i32, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    _get_cached_pattern: usize,
    _get_cached_parent: usize,
    _get_cached_children: usize,
    _current_process_id: usize,
    pub CurrentControlType: unsafe extern "system" fn(*mut core::ffi::c_void, *mut UIA_CONTROLTYPE_ID) -> windows_core::HRESULT,
    _current_localized_control_type: usize,
    pub CurrentName: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    _current_accelerator_key: usize,
    _current_access_key: usize,
    pub CurrentHasKeyboardFocus: unsafe extern "system" fn(*mut core::ffi::c_void, *mut windows_core::BOOL) -> windows_core::HRESULT,
    _current_is_keyboard_focusable: usize,
    pub CurrentIsEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, *mut windows_core::BOOL) -> windows_core::HRESULT,
    _current_automation_id: usize,
    _current_class_name: usize,
    _current_help_text: usize,
    _current_culture: usize,
    _current_is_control_element: usize,
    _current_is_content_element: usize,
    _current_is_password: usize,
    _current_native_window_handle: usize,
    _current_item_type: usize,
    _current_is_offscreen: usize,
    _current_orientation: usize,
    _current_framework_id: usize,
    _current_is_required_for_form: usize,
    _current_item_status: usize,
    pub CurrentBoundingRectangle: unsafe extern "system" fn(*mut core::ffi::c_void, *mut windows_sys::Win32::Foundation::RECT) -> windows_core::HRESULT,
}

#[allow(non_snake_case)]
impl IUIAutomationElement {
    pub unsafe fn FindAll(&self, scope: TreeScope, condition: &IUIAutomationCondition) -> windows_core::Result<IUIAutomationElementArray> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).FindAll)(windows_core::Interface::as_raw(self), scope, condition.as_raw(), &mut result__)
            .and_then(|| windows_core::Type::from_abi(result__))
    }

    pub unsafe fn CurrentControlType(&self) -> windows_core::Result<UIA_CONTROLTYPE_ID> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).CurrentControlType)(windows_core::Interface::as_raw(self), &mut result__)
            .map(|| result__)
    }

    pub unsafe fn CurrentName(&self) -> windows_core::Result<windows_core::BSTR> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).CurrentName)(windows_core::Interface::as_raw(self), &mut result__)
            .map(|| core::mem::transmute(result__))
    }

    pub unsafe fn CurrentIsEnabled(&self) -> windows_core::Result<bool> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).CurrentIsEnabled)(windows_core::Interface::as_raw(self), &mut result__)
            .map(|| result__.0 != 0)
    }

    pub unsafe fn CurrentHasKeyboardFocus(&self) -> windows_core::Result<bool> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).CurrentHasKeyboardFocus)(windows_core::Interface::as_raw(self), &mut result__)
            .map(|| result__.0 != 0)
    }

    pub unsafe fn CurrentBoundingRectangle(&self) -> windows_core::Result<windows_sys::Win32::Foundation::RECT> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).CurrentBoundingRectangle)(windows_core::Interface::as_raw(self), &mut result__)
            .map(|| result__)
    }

    /// 按模式 ID 取当前元素支持的 pattern 接口；元素不支持时返回 Ok(空指针)。
    /// 返回的裸指针即为该 pattern 的接口指针，由调用方按 IID 转型（GetCurrentPattern 保证类型一致）。
    pub unsafe fn GetCurrentPatternRaw(&self, pattern_id: i32) -> windows_core::Result<*mut core::ffi::c_void> {
        let mut raw: *mut core::ffi::c_void = core::ptr::null_mut();
        (windows_core::Interface::vtable(self).GetCurrentPattern)(windows_core::Interface::as_raw(self), pattern_id, &mut raw)
            .map(|| raw)
    }
}

// ==================== IUIAutomation pattern 接口（Invoke / Value / Scroll） ====================
//
// 接口与 IID 复制自 windows-0.61.3 生成代码（Win32::UI::Accessibility），仅保留会用到的方法。
// 模式 ID：Invoke=10000、Value=10002、Scroll=10004；ScrollAmount 枚举值沿用 SDK 官方命名。

/// UIA ScrollAmount 枚举（数值来自 Windows SDK UIA_ScrollAmount）
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ScrollAmount(pub i32);

#[allow(non_upper_case_globals)]
pub const ScrollAmount_LargeDecrement: ScrollAmount = ScrollAmount(0);
#[allow(non_upper_case_globals)]
pub const ScrollAmount_SmallDecrement: ScrollAmount = ScrollAmount(1);
#[allow(non_upper_case_globals)]
pub const ScrollAmount_NoAmount: ScrollAmount = ScrollAmount(2);
#[allow(non_upper_case_globals)]
pub const ScrollAmount_LargeIncrement: ScrollAmount = ScrollAmount(3);
#[allow(non_upper_case_globals)]
pub const ScrollAmount_SmallIncrement: ScrollAmount = ScrollAmount(4);

define_interface!(IUIAutomationInvokePattern, IUIAutomationInvokePattern_Vtbl, 0xfb377fbe_8ea6_46d5_9c73_6499642d3059);
interface_hierarchy!(IUIAutomationInvokePattern, windows_core::IUnknown);

#[allow(non_snake_case)]
#[repr(C)]
pub struct IUIAutomationInvokePattern_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Invoke: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}

#[allow(non_snake_case)]
impl IUIAutomationInvokePattern {
    pub unsafe fn Invoke(&self) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).Invoke)(windows_core::Interface::as_raw(self)).map(|| ())
    }
}

define_interface!(IUIAutomationValuePattern, IUIAutomationValuePattern_Vtbl, 0xa94cd8b1_0844_4cd6_9d2d_640537ab39e9);
interface_hierarchy!(IUIAutomationValuePattern, windows_core::IUnknown);

#[allow(non_snake_case)]
#[repr(C)]
pub struct IUIAutomationValuePattern_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub SetValue: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub get_CurrentValue: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    _get_cached_value: usize,
    _get_is_read_only: usize,
    _get_cached_is_read_only: usize,
}

#[allow(non_snake_case)]
impl IUIAutomationValuePattern {
    /// SetValue 接收 BSTR；传入的 BSTR 由调用方持有并保持存活到调用返回。
    pub unsafe fn SetValue(&self, value: *mut core::ffi::c_void) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).SetValue)(windows_core::Interface::as_raw(self), value).map(|| ())
    }

    /// 读回控件当前值（BSTR 所有权转移给返回值）。
    pub unsafe fn CurrentValue(&self) -> windows_core::Result<windows_core::BSTR> {
        let mut value = core::ptr::null_mut();
        (windows_core::Interface::vtable(self).get_CurrentValue)(windows_core::Interface::as_raw(self), &mut value)
            .map(|| unsafe { windows_core::BSTR::from_raw(value as *const u16) })
    }
}

define_interface!(IUIAutomationScrollPattern, IUIAutomationScrollPattern_Vtbl, 0x88f4d42a_e881_459d_a77c_73bbbb7e02dc);
interface_hierarchy!(IUIAutomationScrollPattern, windows_core::IUnknown);

#[allow(non_snake_case)]
#[repr(C)]
pub struct IUIAutomationScrollPattern_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Scroll: unsafe extern "system" fn(*mut core::ffi::c_void, ScrollAmount, ScrollAmount) -> windows_core::HRESULT,
}

#[allow(non_snake_case)]
impl IUIAutomationScrollPattern {
    pub unsafe fn Scroll(&self, horizontal: ScrollAmount, vertical: ScrollAmount) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).Scroll)(windows_core::Interface::as_raw(self), horizontal, vertical).map(|| ())
    }
}

// ==================== IUIAutomationElementArray ====================

define_interface!(IUIAutomationElementArray, IUIAutomationElementArray_Vtbl, 0x14314595_b4bc_4055_95f2_58f2e42c9855);
interface_hierarchy!(IUIAutomationElementArray, windows_core::IUnknown);

#[allow(non_snake_case)]
#[repr(C)]
pub struct IUIAutomationElementArray_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Length: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub GetElement: unsafe extern "system" fn(*mut core::ffi::c_void, i32, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}

#[allow(non_snake_case)]
impl IUIAutomationElementArray {
    pub unsafe fn Length(&self) -> windows_core::Result<i32> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).Length)(windows_core::Interface::as_raw(self), &mut result__)
            .map(|| result__)
    }

    pub unsafe fn GetElement(&self, index: i32) -> windows_core::Result<IUIAutomationElement> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).GetElement)(windows_core::Interface::as_raw(self), index, &mut result__)
            .and_then(|| windows_core::Type::from_abi(result__))
    }
}

// ==================== IUIAutomationCondition（空接口） ====================

define_interface!(IUIAutomationCondition, IUIAutomationCondition_Vtbl, 0x352ffba8_0973_437c_a61f_f64cafd81df9);
interface_hierarchy!(IUIAutomationCondition, windows_core::IUnknown);

#[allow(non_snake_case)]
#[repr(C)]
pub struct IUIAutomationCondition_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
}

// ==================== 鼠标注入 ====================

/// SendInput 绝对坐标单轴归一化：0~65535 覆盖整块虚拟桌面（带 MOUSEEVENTF_VIRTUALDESK）。
/// value 为虚拟桌面物理像素，origin/extent 为虚拟桌面在该轴上的原点与边长。
fn normalize_absolute_axis(value: i32, origin: i32, extent: i32) -> i32 {
    let span = extent as i64 - 1;
    if span <= 0 {
        return 0;
    }
    (((value - origin) as i64 * 65535 + span / 2) / span) as i32
}

/// 绝对鼠标移动：坐标为虚拟桌面物理像素。
/// enigo 的 Coordinate::Abs 按主屏尺寸归一化且不带 MOUSEEVENTF_VIRTUALDESK，
/// 主屏之外的合法坐标会被系统静默钳到主屏边缘，所以这里自己发 SendInput。
pub fn mouse_move_absolute(x: i32, y: i32) -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
        MOUSEEVENTF_VIRTUALDESK,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    };

    unsafe {
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if vw <= 1 || vh <= 1 {
            return Err(format!("读取虚拟桌面尺寸失败：{vw}x{vh}"));
        }
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: normalize_absolute_axis(x, vx, vw),
                    dy: normalize_absolute_axis(y, vy, vh),
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let sent = SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
        if sent != 1 {
            return Err(format!("SendInput 注入鼠标移动失败：sent={sent}"));
        }
    }
    Ok(())
}

// ==================== 遍历入口 ====================

/// 批量扫描指定窗口列表的可交互元素树（一次 COM/automation 实例复用）。
/// windows 为 (hwnd, 窗口标题) 列表；返回扁平元素列表（虚拟桌面物理像素坐标，带 window 字段）。
pub fn collect_ui_tree_for_windows(
    windows: &[(usize, String)],
    include_text: bool,
) -> Vec<UiElementInfo> {
    if windows.is_empty() {
        return Vec::new();
    }
    let mut all = Vec::with_capacity(64);

    // 不解构元组：字段按声明顺序释放，IUIAutomation 必须先于 ComGuard（CoUninitialize）释放；
    // 先卸载 COM 再 Release 已创建的 UIA 对象会触发访问冲突。
    let session = match create_uia_automation() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    for (hwnd, title) in windows {
        if all.len() >= MAX_ELEMENTS {
            break;
        }
        let remaining = MAX_ELEMENTS - all.len();
        let mut scanned = scan_window(&session.0, *hwnd, remaining, include_text);
        for elem in scanned.iter_mut() {
            elem.window_id = *hwnd as u32;
            elem.window_title = title.clone();
        }
        all.append(&mut scanned);
    }
    all
}

/// 对指定窗口句柄扫描可交互元素树。
/// hwnd 为 0 时忽略该窗口（失败跳过）；返回扁平元素列表（虚拟桌面物理像素坐标）。
pub fn collect_window_ui_elements(hwnd: usize, include_text: bool) -> Vec<UiElementInfo> {
    collect_ui_tree_for_windows(&[(hwnd, String::new())], include_text)
}

/// 扫描到的原始元素（保留 COM 对象与原始屏幕坐标，供 pattern 调用复用）
#[derive(Clone)]
struct RawUiElement {
    element: IUIAutomationElement,
    type_name: &'static str,
    name: String,
    rect: windows_sys::Win32::Foundation::RECT,
    focused: bool,
}

fn scan_window(
    automation: &IUIAutomation,
    hwnd: usize,
    max_elements: usize,
    include_text: bool,
) -> Vec<UiElementInfo> {
    if hwnd == 0 || max_elements == 0 {
        return Vec::new();
    }
    // 最小化窗口没有真实屏幕位置，UIA 会返回垃圾坐标，直接跳过
    unsafe {
        if windows_sys::Win32::UI::WindowsAndMessaging::IsIconic(hwnd as _) != 0 {
            return Vec::new();
        }
    }
    collect_raw_elements(automation, hwnd, max_elements, include_text)
        .into_iter()
        .map(|raw| UiElementInfo {
            window_id: hwnd as u32,
            window_title: String::new(),
            control_type: raw.type_name.to_string(),
            name: raw.name,
            x: raw.rect.left,
            y: raw.rect.top,
            width: raw.rect.right - raw.rect.left,
            height: raw.rect.bottom - raw.rect.top,
            focused: raw.focused,
        })
        .collect()
}

/// 对指定窗口做一次可交互元素扫描（保留 COM 元素对象）；过滤与排序语义与 scan_window 一致：
/// 只保留白名单类型、enabled、矩形非空的元素，顺序为 FindAll 返回顺序。
///
/// 失败一律返回已收集到的部分（可能是空表）：调用方按「没有元素」处理。
/// 因此每次失败的 HRESULT 都要记下来——协议层超时和「窗口没有暴露 UIA」在上层看到的结果完全相同，
/// 只有这里的日志能区分，否则排障时无从判断是窗口卡住还是控件真的不暴露。
fn collect_raw_elements(
    automation: &IUIAutomation,
    hwnd: usize,
    max_elements: usize,
    include_text: bool,
) -> Vec<RawUiElement> {
    let mut elements = Vec::with_capacity(max_elements.min(64));
    unsafe {
        let root = match automation.ElementFromHandle(hwnd as *mut core::ffi::c_void) {
            Ok(e) => e,
            Err(err) => {
                runtime_log_warn(format!(
                    "[控件树] 取窗口根元素失败，任务=collect_raw_elements，window_id=0x{hwnd:x}，HRESULT={err:?}（超时 or 窗口未响应）"
                ));
                return elements;
            }
        };
        let condition = match automation.CreateTrueCondition() {
            Ok(c) => c,
            Err(err) => {
                runtime_log_warn(format!(
                    "[控件树] 创建遍历条件失败，任务=collect_raw_elements，window_id=0x{hwnd:x}，HRESULT={err:?}"
                ));
                return elements;
            }
        };
        let array = match root.FindAll(TreeScope_Descendants, &condition) {
            Ok(a) => a,
            Err(err) => {
                runtime_log_warn(format!(
                    "[控件树] 遍历元素失败，任务=collect_raw_elements，window_id=0x{hwnd:x}，HRESULT={err:?}（重负载窗口可能触发事务超时）"
                ));
                return elements;
            }
        };
        let len = array.Length().unwrap_or(0);
        for i in 0..len {
            if elements.len() >= max_elements {
                break;
            }
            let elem = match array.GetElement(i) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let ct = match elem.CurrentControlType() {
                Ok(v) => v,
                Err(_) => continue,
            };
            // 默认只保留可交互控件；text=true 时额外带上 Text 控件（界面文字标签），
            // 供模型用上下文定位目标。app 动作的元素序号路径固定传 false，避免序号被文本撑变。
            if !is_interactive_control_type(ct) && !(include_text && ct == UIA_TextControlTypeId) {
                continue;
            }
            let enabled = elem.CurrentIsEnabled().unwrap_or(false);
            if !enabled {
                continue;
            }
            let rect = match elem.CurrentBoundingRectangle() {
                Ok(r) => r,
                Err(_) => continue,
            };
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;
            if w <= 0 || h <= 0 {
                continue;
            }
            let name = elem
                .CurrentName()
                .ok()
                .map(|b| b.to_string())
                .unwrap_or_default();
            let focused = elem.CurrentHasKeyboardFocus().unwrap_or(false);
            elements.push(RawUiElement {
                element: elem,
                type_name: control_type_name(ct),
                name,
                rect,
                focused,
            });
        }
    }
    elements
}

/// COM 初始化守卫：仅当本代码首次初始化 COM（S_OK）时，Drop 时配平 CoUninitialize。
/// S_FALSE / RPC_E_CHANGED_MODE（线程已初始化）不执行清理，panic 时也会正确释放。
struct ComGuard {
    owns_com: bool,
}

impl ComGuard {
    fn new(owns_com: bool) -> Self {
        Self { owns_com }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.owns_com {
            unsafe {
                windows_sys::Win32::System::Com::CoUninitialize();
            }
        }
    }
}

// ==================== app 动作族（后台点击 / 写值 / 滚动） ====================

// UIA pattern ID（UIA_InvokePatternId / UIA_ValuePatternId / UIA_ScrollPatternId）
const UIA_INVOKE_PATTERN_ID: i32 = 10000;
const UIA_VALUE_PATTERN_ID: i32 = 10002;
const UIA_SCROLL_PATTERN_ID: i32 = 10004;

/// UIA 协议层超时（毫秒）。两个超时职责不同，取值也不同：
/// - 连接超时管「能不能连上目标窗口的 provider」，系统默认 2 秒。这里取 2 秒，让它在多数情况下
///   先于外层单次调用限时（operate_blocking 的 3 秒）失败，模型能拿到 ERROR_TIMEOUT 而不是笼统的
///   「平台调用超时」。
/// - 事务超时管「provider 处理一批元素要多久」，系统默认 20 秒（微软文档明确说有些操作要处理上百个
///   元素）。本项目单次 FindAll 最多取 MAX_ELEMENTS 个元素，全部降到 3 秒会让重负载窗口扫描失败并
///   退化成空树。这里取 10 秒：仍比默认严格一半，但足以覆盖数百元素的正常扫描。
///   它比外层 3 秒更长是有意的——外层超时只是不再等待，阻塞中的线程仍在；事务超时让那个线程最终
///   能返回并释放，避免长时间占用阻塞线程池。
/// 0 表示无限等待，这里刻意不暴露成可配置项。
const UIA_CONNECTION_TIMEOUT_MS: u32 = 2_000;
const UIA_TRANSACTION_TIMEOUT_MS: u32 = 10_000;

/// 创建 UIA 实例（COM 初始化 + CoCreateInstance）；COM 守卫随返回值存活到调用结束。
fn create_uia_automation() -> Result<(IUIAutomation, ComGuard), String> {
    // COM 初始化：S_OK=本代码首次初始化（需配平）；S_FALSE=线程已初始化（复用，无需配平）；
    // RPC_E_CHANGED_MODE=线程已以其他模式初始化（COM 仍可用，无需配平）。
    let coinit = unsafe { windows_sys::Win32::System::Com::CoInitializeEx(core::ptr::null(), 0 /* COINIT_MULTITHREADED */) };
    let owns_com = coinit == 0;
    let com_ok = coinit >= 0 || coinit == windows_sys::Win32::Foundation::RPC_E_CHANGED_MODE;
    if !com_ok {
        return Err(format!("COM 初始化失败：HRESULT={coinit}"));
    }
    let guard = ComGuard::new(owns_com);
    let automation = create_uia_with_timeouts()?;
    Ok((automation, guard))
}

/// 优先用 CUIAutomation8 拿到 IUIAutomation2 并设置协议层超时；系统不支持时退回 CUIAutomation。
/// CUIAutomation8 只在 Windows 8 及以上提供，Windows 7 上会创建失败，此时退回 v1（无超时，行为同旧版）。
fn create_uia_with_timeouts() -> Result<IUIAutomation, String> {
    // CLSID_CUIAutomation8 / IID_IUIAutomation2
    let clsid_v2 = windows_sys::core::GUID::from_u128(0xe22ad333_b25f_460c_83d0_0581107395c9);
    let iid_v2 = windows_sys::core::GUID::from_u128(0x34723aff_0c9d_49d0_9896_7ab52df8cd8a);
    let mut raw: *mut core::ffi::c_void = core::ptr::null_mut();
    let hr = unsafe {
        windows_sys::Win32::System::Com::CoCreateInstance(
            &clsid_v2,
            core::ptr::null_mut(),
            windows_sys::Win32::System::Com::CLSCTX_INPROC_SERVER,
            &iid_v2,
            &mut raw,
        )
    };
    if hr >= 0 && !raw.is_null() {
        let automation2 = unsafe { IUIAutomation2::from_raw(raw) };
        // 设置失败不致命：只是没有协议层超时保护，继续用 v1 的调用面。
        unsafe {
            if automation2.SetConnectionTimeout(UIA_CONNECTION_TIMEOUT_MS).is_err() {
                runtime_log_warn(format!(
                    "[控件树] 设置 UIA 连接超时失败，继续执行，任务=create_uia_with_timeouts，timeout_ms={UIA_CONNECTION_TIMEOUT_MS}"
                ));
            }
            if automation2.SetTransactionTimeout(UIA_TRANSACTION_TIMEOUT_MS).is_err() {
                runtime_log_warn(format!(
                    "[控件树] 设置 UIA 事务超时失败，继续执行，任务=create_uia_with_timeouts，timeout_ms={UIA_TRANSACTION_TIMEOUT_MS}"
                ));
            }
        }
        // IUIAutomation2 继承 IUIAutomation，这里交出基接口指针供既有调用面使用。
        // cast 走 QueryInterface，引用计数升到 2；automation2 在本函数返回时析构降回 1，
        // 交出的 base 独占一个引用，不存在悬垂。
        match automation2.cast::<IUIAutomation>() {
            Ok(base) => return Ok(base),
            Err(err) => {
                // 拿不到基接口说明这个 COM 对象不可用；既然已有可用的旧路径，退回比整条 UIA 失效更好。
                runtime_log_warn(format!(
                    "[控件树] IUIAutomation2 转 IUIAutomation 失败，退回 CUIAutomation，任务=create_uia_with_timeouts，原因={err}"
                ));
                return create_legacy_uia_automation();
            }
        }
    }

    runtime_log_info(format!(
        "[控件树] CUIAutomation8 不可用，退回无超时的 CUIAutomation，任务=create_uia_with_timeouts，HRESULT={hr}"
    ));
    create_legacy_uia_automation()
}

fn create_legacy_uia_automation() -> Result<IUIAutomation, String> {
    let clsid = windows_sys::core::GUID::from_u128(0xff48dba4_60ef_4201_aa87_54103eef594e); // CLSID_CUIAutomation
    let iid = windows_sys::core::GUID::from_u128(0x30cbe57d_d9d0_452a_ab13_7ac5ac4825ee); // IID_IUIAutomation
    let mut automation_raw: *mut core::ffi::c_void = core::ptr::null_mut();
    let hr = unsafe {
        windows_sys::Win32::System::Com::CoCreateInstance(
            &clsid,
            core::ptr::null_mut(),
            windows_sys::Win32::System::Com::CLSCTX_INPROC_SERVER,
            &iid,
            &mut automation_raw,
        )
    };
    if hr < 0 || automation_raw.is_null() {
        return Err(format!("创建 UIA 实例失败：HRESULT={hr}"));
    }
    Ok(unsafe { IUIAutomation::from_raw(automation_raw) })
}

fn with_uia_automation<T>(f: impl FnOnce(&IUIAutomation) -> Result<T, String>) -> Result<T, String> {
    // 不解构元组：automation 必须先于 COM 守卫释放（见 collect_ui_tree_for_windows 注释）
    let session = create_uia_automation()?;
    f(&session.0)
}

/// 核对名字寻址元素的执行前状态：与解析时新鲜扫描的记录比对。
/// 名字寻址每次执行都重新扫描，这里只防扫描与执行之间的竞态（页面恰好刷新），
/// 所以不一致时建议重试一次，而不是重新截图。
/// 名字按归一后比对（解析时返回的也是归一后名字），展示同样用归一结果。
fn verify_named_element(raw: &RawUiElement, control_type: &str, name: &str, ordinal: usize) -> Result<(), String> {
    if raw.type_name != control_type {
        return Err(format!(
            "元素 `{name}`（窗口内第 {} 项）已变化：现为 {}('{}')；页面可能刚刷新，请重试一次",
            ordinal + 1,
            raw.type_name,
            normalize_element_name(&raw.name)
        ));
    }
    if !name.is_empty() && normalize_element_name(&raw.name) != name {
        return Err(format!(
            "元素 `{name}`（窗口内第 {} 项）名称变为 '{}'；页面可能刚刷新，请重试一次",
            ordinal + 1,
            normalize_element_name(&raw.name)
        ));
    }
    Ok(())
}

/// 元素名归一：首尾空白去掉、内部换行/制表/连续空白折成单个空格。
/// 匹配与展示共用同一套归一，模型照抄展示出来的名字就能命中（否则差一个空格就找不到）。
fn normalize_element_name(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 未命中时的候选展示（P0-2）：按控件类型分组，每类最多 3 个、最多 6 类。
/// 按类型分组而不是按扫描顺序取前 N 个，是为了让页面内容元素（Hyperlink/Button 等）
/// 不会被窗口边框与工具栏元素挤掉——扫描顺序天然把框架元素排在最前面。
/// 全部元素都没有可读名时返回 None，由调用方换成「均无可读名」的说明。
fn render_candidate_groups(list: &[RawUiElement]) -> Option<String> {
    render_candidate_groups_from(list.iter().map(|e| (e.type_name, e.name.as_str())))
}

/// 候选分组渲染的纯逻辑：输入 (控件类型, 原始名) 序列，输出分组文案。
/// 与 UIA 扫描解耦，便于单测覆盖分组、每类上限与类型数上限。
fn render_candidate_groups_from<'a>(elements: impl Iterator<Item = (&'static str, &'a str)>) -> Option<String> {
    let mut groups = std::collections::BTreeMap::<&'static str, Vec<String>>::new();
    for (type_name, raw_name) in elements {
        let display = normalize_element_name(raw_name);
        if display.is_empty() {
            continue;
        }
        let names = groups.entry(type_name).or_default();
        if names.len() < 3 {
            names.push(display);
        }
    }
    if groups.is_empty() {
        return None;
    }
    let group_count = groups.len();
    let rendered = groups
        .iter()
        .take(6)
        .map(|(type_name, names)| format!("{type_name}：{}", names.join("、")))
        .collect::<Vec<_>>()
        .join("；");
    let more = if group_count > 6 { format!("…（共 {group_count} 类）") } else { String::new() };
    Some(format!("{rendered}{more}"))
}

/// 按元素名解析窗口内控件（新鲜扫描、唯一命中）：返回 (窗口内序号, 类型, 全名)。
/// 找不到或命中多个时列出候选，供模型自我纠正；无名元素请用窗口内坐标兜底。
/// 候选按控件类型分组展示（每类最多 3 个、最多 6 类），保证页面内容元素能进候选，
/// 而不是被窗口边框与工具栏元素按扫描顺序挤掉。
pub fn app_find_element_by_name(hwnd: usize, needle: &str) -> Result<(usize, String, String), String> {
    with_uia_automation(|automation| {
        let list = collect_raw_elements(automation, hwnd, MAX_ELEMENTS, false);
        if list.is_empty() {
            return Err(format!("窗口内没有可交互元素，无法按名查找 `{needle}`；请改用坐标兜底（w click @x,y）"));
        }
        let key = normalize_element_name(needle).to_lowercase();
        let matched = list
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let display = normalize_element_name(&e.name);
                !display.is_empty() && display.to_lowercase().contains(&key)
            })
            .collect::<Vec<_>>();
        match matched.as_slice() {
            [(ordinal, raw)] => Ok((*ordinal, raw.type_name.to_string(), normalize_element_name(&raw.name))),
            [] => {
                match render_candidate_groups(&list) {
                    Some(rendered) => return Err(format!("没有名为 `{needle}` 的元素；窗口内现有（按类型分组，每类最多 3 个）：{rendered}；请检查名字或改用坐标兜底（w click @x,y）")),
                    None => return Err(format!("没有名为 `{needle}` 的元素：窗口内元素均无可读名；请改用坐标兜底（w click @x,y）")),
                }
            }
            many => {
                let candidates = many
                    .iter()
                    .take(8)
                    .map(|(_, e)| format!("{}('{}')", e.type_name, normalize_element_name(&e.name)))
                    .collect::<Vec<_>>()
                    .join("、");
                Err(format!("名为 `{needle}` 的元素有 {} 个：{}；请用更精确的名字，或改用坐标兜底（w click @x,y）", many.len(), candidates))
            }
        }
    })
}

/// 解析 app 目标：NamedElement 按新鲜扫描序号核对（名字寻址）；
/// Point 命中包含该坐标的最小交互元素（无命中为 None）。返回 (元素, 事件屏幕坐标)。
fn resolve_app_target(
    automation: &IUIAutomation,
    hwnd: usize,
    target: &AppTarget,
) -> Result<(Option<RawUiElement>, (i32, i32)), String> {
    match target {
        AppTarget::NamedElement { ordinal, control_type, name } => {
            let list = collect_raw_elements(automation, hwnd, MAX_ELEMENTS, false);
            let raw = list.get(*ordinal).ok_or_else(|| {
                format!(
                    "元素 `{name}` 已消失：当前控件树共 {} 项，页面可能刚刷新，请重试一次（每次执行都会重新扫描）",
                    list.len()
                )
            })?;
            verify_named_element(raw, control_type, name, *ordinal)?;
            let point = ((raw.rect.left + raw.rect.right) / 2, (raw.rect.top + raw.rect.bottom) / 2);
            Ok((Some(raw.clone()), point))
        }
        AppTarget::Point { screen_x, screen_y } => {
            let list = collect_raw_elements(automation, hwnd, MAX_ELEMENTS, false);
            let hit = list
                .iter()
                .filter(|e| {
                    e.rect.left <= *screen_x && *screen_x < e.rect.right && e.rect.top <= *screen_y && *screen_y < e.rect.bottom
                })
                .min_by_key(|e| (e.rect.right - e.rect.left) * (e.rect.bottom - e.rect.top))
                .cloned();
            Ok((hit, (*screen_x, *screen_y)))
        }
    }
}

/// 后台点击：UIA InvokePattern 优先，元素不支持或坐标无元素命中时降级 PostMessage 投递鼠标消息。
/// dblclick=true 时跳过 Invoke（Invoke 是语义激活，无双击概念）直接走 PostMessage 双击序列。
/// 不移动全局光标、不抢焦点。降级路径在 downgrade 里说明「消息发出去了，但未确认生效」。
pub fn app_click(hwnd: usize, target: &AppTarget, repeat: u32, dblclick: bool) -> Result<AppClickOutcome, String> {
    with_uia_automation(|automation| {
        let (element, point) = resolve_app_target(automation, hwnd, target)?;
        let hit_control_type = element.as_ref().map(|raw| raw.type_name);
        if let Some(raw) = &element {
            if !dblclick {
                unsafe {
                    if let Ok(pattern_raw) = raw.element.GetCurrentPatternRaw(UIA_INVOKE_PATTERN_ID) {
                        if !pattern_raw.is_null() {
                            let pattern: IUIAutomationInvokePattern = windows_core::Type::from_abi(pattern_raw)
                                .map_err(|err| format!("转换 InvokePattern 失败：{err}"))?;
                            for _ in 0..repeat.max(1) {
                                pattern.Invoke().map_err(|err| format!("Invoke 调用失败：{err}"))?;
                            }
                            return Ok(AppClickOutcome { method: "invoke", downgrade: None, hit_control_type });
                        }
                    }
                }
            }
        }
        post_mouse_click(hwnd, point.0, point.1, repeat, dblclick)?;
        let downgrade = match (&element, dblclick) {
            (_, true) => "dblclick 走坐标投递，结果不保证".to_string(),
            (Some(raw), false) => format!("{}('{}') 无 Invoke 支持，已退化为坐标投递，结果不保证", raw.type_name, raw.name),
            (None, false) => "坐标未命中元素，已按坐标投递，结果不保证".to_string(),
        };
        Ok(AppClickOutcome { method: "postmessage", downgrade: Some(downgrade), hit_control_type })
    })
}

/// 后台读值：ValuePattern.CurrentValue 读回目标文本控件当前值。无 PostMessage 降级。
pub fn app_get_value(hwnd: usize, target: &AppTarget) -> Result<String, String> {
    with_uia_automation(|automation| {
        if !matches!(target, AppTarget::NamedElement { .. }) {
            return Err("getvalue 必须使用元素名指定控件，例如：w getvalue \"用户名\"".to_string());
        }
        let (element, _) = resolve_app_target(automation, hwnd, target)?;
        let raw = element.ok_or_else(|| "getvalue 必须使用元素名指定控件，例如：w getvalue \"用户名\"".to_string())?;
        unsafe {
            let pattern_raw = raw
                .element
                .GetCurrentPatternRaw(UIA_VALUE_PATTERN_ID)
                .map_err(|err| format!("查询 ValuePattern 失败：{err}"))?;
            if pattern_raw.is_null() {
                return Err(format!("目标控件（{}）不支持 ValuePattern，无法后台读取值", raw.type_name));
            }
            let pattern: IUIAutomationValuePattern = windows_core::Type::from_abi(pattern_raw)
                .map_err(|err| format!("转换 ValuePattern 失败：{err}"))?;
            pattern
                .CurrentValue()
                .map(|bstr| bstr.to_string())
                .map_err(|err| format!("CurrentValue 调用失败：{err}"))
        }
    })
}

/// 查询目标窗口线程当前内部焦点控件（type, name）。
/// GetGUIThreadInfo 对后台窗口同样有效；hwndFocus 为空（窗口无内部焦点）或查询失败返回 None。
pub fn app_focus_summary(hwnd: usize) -> Result<Option<(String, String)>, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO};

    if hwnd == 0 {
        return Ok(None);
    }
    let mut pid = 0u32;
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd as _, &mut pid) };
    if thread_id == 0 {
        return Ok(None);
    }
    let mut info: GUITHREADINFO = unsafe { core::mem::zeroed() };
    info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if unsafe { GetGUIThreadInfo(thread_id, &mut info) } == 0 || info.hwndFocus.is_null() {
        return Ok(None);
    }
    match with_uia_automation(|automation| {
        unsafe {
            let element = automation
                .ElementFromHandle(info.hwndFocus as *mut core::ffi::c_void)
                .map_err(|err| format!("焦点控件转换为 UIA 元素失败：{err}"))?;
            let type_name = element
                .CurrentControlType()
                .map(control_type_name)
                .unwrap_or("Unknown");
            let name = element.CurrentName().ok().map(|b| b.to_string()).unwrap_or_default();
            Ok(Some((type_name.to_string(), name)))
        }
    }) {
        Ok(v) => Ok(v),
        // 焦点摘要只是附加上下文，失败静默降级，不影响动作本身
        Err(_) => Ok(None),
    }
}

/// 后台写值：ValuePattern.SetValue 整体替换目标文本控件内容。无 PostMessage 降级
/// （向任意控件盲投 WM_SETTEXT 风险大，不支持 pattern 的控件应明确报错）。
/// 返回实际使用的投递方式："valuepattern"。
pub fn app_set_value(hwnd: usize, target: &AppTarget, text: &str) -> Result<&'static str, String> {
    with_uia_automation(|automation| {
        if !matches!(target, AppTarget::NamedElement { .. }) {
            return Err("setvalue 必须使用元素名指定文本控件，例如：w setvalue \"用户名\" = \"admin\"".to_string());
        }
        let (element, _) = resolve_app_target(automation, hwnd, target)?;
        let raw = element.ok_or_else(|| "setvalue 必须使用元素名指定文本控件，例如：w setvalue \"用户名\" = \"admin\"".to_string())?;
        unsafe {
            let pattern_raw = raw
                .element
                .GetCurrentPatternRaw(UIA_VALUE_PATTERN_ID)
                .map_err(|err| format!("查询 ValuePattern 失败：{err}"))?;
            if pattern_raw.is_null() {
                return Err("目标控件不支持 ValuePattern，无法后台写入文本".to_string());
            }
            let pattern: IUIAutomationValuePattern = windows_core::Type::from_abi(pattern_raw)
                .map_err(|err| format!("转换 ValuePattern 失败：{err}"))?;
            let bstr = windows_core::BSTR::from(text);
            pattern
                .SetValue(bstr.as_ptr() as *mut core::ffi::c_void)
                .map_err(|err| format!("SetValue 调用失败：{err}"))?;
        }
        Ok("valuepattern")
    })
}

/// 后台滚动：ScrollPattern 优先，不支持时降级 PostMessage WM_MOUSEWHEEL。
/// 方向语义与滚轮一致：up=视图上移（内容回退），down=视图下移（内容前进）。
/// 返回实际使用的投递方式："scrollpattern" 或 "postmessage"。
pub fn app_scroll(hwnd: usize, target: &AppTarget, horizontal: bool, positive: bool, small: bool, repeat: u32) -> Result<&'static str, String> {
    with_uia_automation(|automation| {
        let (element, point) = resolve_app_target(automation, hwnd, target)?;
        // positive=true 表示 down（垂直）/ right（水平），与 enigo scroll 符号约定一致；
        // UIA 上对应 LargeIncrement，反之为 LargeDecrement。
        let amount = match (positive, small) {
            (true, false) => ScrollAmount_LargeIncrement,
            (true, true) => ScrollAmount_SmallIncrement,
            (false, false) => ScrollAmount_LargeDecrement,
            (false, true) => ScrollAmount_SmallDecrement,
        };
        if let Some(raw) = &element {
            unsafe {
                if let Ok(pattern_raw) = raw.element.GetCurrentPatternRaw(UIA_SCROLL_PATTERN_ID) {
                    if !pattern_raw.is_null() {
                        let pattern: IUIAutomationScrollPattern = windows_core::Type::from_abi(pattern_raw)
                            .map_err(|err| format!("转换 ScrollPattern 失败：{err}"))?;
                        for _ in 0..repeat.max(1) {
                            let (h, v) = if horizontal {
                                (amount, ScrollAmount_NoAmount)
                            } else {
                                (ScrollAmount_NoAmount, amount)
                            };
                            pattern
                                .Scroll(h, v)
                                .map_err(|err| format!("Scroll 调用失败：{err}"))?;
                        }
                        return Ok("scrollpattern");
                    }
                }
            }
        }
        if horizontal {
            post_mouse_hwheel(hwnd, point.0, point.1, positive, repeat)?;
        } else {
            post_mouse_wheel(hwnd, point.0, point.1, !positive, repeat)?;
        }
        Ok("postmessage")
    })
}

/// 从顶层窗口逐层下沉到屏幕坐标命中的子窗口（RealChildWindowFromPoint 自动跳过透明窗口）。
/// 返回 (最终子窗口, 该子窗口客户区坐标)。
fn descend_to_child_at(
    hwnd: usize,
    screen_x: i32,
    screen_y: i32,
) -> (windows_sys::Win32::Foundation::HWND, windows_sys::Win32::Foundation::POINT) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::WindowsAndMessaging::RealChildWindowFromPoint;

    let mut current = hwnd as _;
    for _ in 0..16 {
        let mut client = POINT { x: screen_x, y: screen_y };
        unsafe {
            ScreenToClient(current, &mut client);
        }
        let child = unsafe { RealChildWindowFromPoint(current, client) };
        if child.is_null() || child == current {
            break;
        }
        current = child;
    }
    let mut client = POINT { x: screen_x, y: screen_y };
    unsafe {
        ScreenToClient(current, &mut client);
    }
    (current, client)
}

/// 鼠标消息 lParam 打包：低 16 位 x、高 16 位 y（客户区坐标，有符号溢出按 u16 截断与 Win32 一致）。
fn mouse_lparam(client_x: i32, client_y: i32) -> isize {
    ((client_y as u16 as isize) << 16) | (client_x as u16 as isize)
}

/// PostMessage 兜底点击：向坐标命中的最深子窗口投递鼠标消息（不移动全局光标、不抢焦点）。
/// dblclick=true 时发送标准双击序列（DOWN/UP → LBUTTONDBLCLK/UP）。
/// UWP/WebView2 等走独立输入管线的界面不响应此类消息，这类界面必须依赖 UIA pattern 路径。
fn post_mouse_click(hwnd: usize, screen_x: i32, screen_y: i32, repeat: u32, dblclick: bool) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, PostMessageW, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    };
    if hwnd == 0 {
        return Err("目标窗口句柄为 0".to_string());
    }
    unsafe {
        if IsIconic(hwnd as _) != 0 {
            return Err("目标窗口已最小化，无法投递鼠标消息".to_string());
        }
    }
    let (target_hwnd, client) = descend_to_child_at(hwnd, screen_x, screen_y);
    let lparam = mouse_lparam(client.x, client.y);
    unsafe {
        for _ in 0..repeat.max(1) {
            PostMessageW(target_hwnd, WM_MOUSEMOVE, 0, lparam);
            PostMessageW(target_hwnd, WM_LBUTTONDOWN, 0x0001 /* MK_LBUTTON */, lparam);
            PostMessageW(target_hwnd, WM_LBUTTONUP, 0, lparam);
            if dblclick {
                PostMessageW(target_hwnd, WM_LBUTTONDBLCLK, 0x0001 /* MK_LBUTTON */, lparam);
                PostMessageW(target_hwnd, WM_LBUTTONUP, 0, lparam);
            }
        }
    }
    Ok(())
}

/// PostMessage 兜底滚动：向坐标命中的子窗口投递 WM_MOUSEWHEEL（lParam 为屏幕坐标，wParam 高 16 位 wheel delta）。
fn post_mouse_wheel(hwnd: usize, screen_x: i32, screen_y: i32, up: bool, repeat: u32) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, PostMessageW, WM_MOUSEWHEEL};
    if hwnd == 0 {
        return Err("目标窗口句柄为 0".to_string());
    }
    unsafe {
        if IsIconic(hwnd as _) != 0 {
            return Err("目标窗口已最小化，无法投递滚轮消息".to_string());
        }
    }
    let (target_hwnd, _) = descend_to_child_at(hwnd, screen_x, screen_y);
    let delta: u16 = if up { 120 } else { u16::MAX - 119 }; // wheel delta：+120 向上，-120 向下（补码）
    let wparam: usize = (delta as usize) << 16;
    let lparam = mouse_lparam(screen_x, screen_y);
    unsafe {
        for _ in 0..repeat.max(1) {
            PostMessageW(target_hwnd, WM_MOUSEWHEEL, wparam, lparam);
        }
    }
    Ok(())
}

/// 水平滚轮兜底（WM_MOUSEHWHEEL = 0x020E）：+120 向右，-120 向左。目标不支持水平滚动时系统静默丢弃。
fn post_mouse_hwheel(hwnd: usize, screen_x: i32, screen_y: i32, right: bool, repeat: u32) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, PostMessageW, WM_MOUSEHWHEEL};
    if hwnd == 0 {
        return Err("目标窗口句柄为 0".to_string());
    }
    unsafe {
        if IsIconic(hwnd as _) != 0 {
            return Err("目标窗口已最小化，无法投递滚轮消息".to_string());
        }
    }
    let (target_hwnd, _) = descend_to_child_at(hwnd, screen_x, screen_y);
    let delta: u16 = if right { 120 } else { u16::MAX - 119 };
    let wparam: usize = (delta as usize) << 16;
    let lparam = mouse_lparam(screen_x, screen_y);
    unsafe {
        for _ in 0..repeat.max(1) {
            PostMessageW(target_hwnd, WM_MOUSEHWHEEL, wparam, lparam);
        }
    }
    Ok(())
}

// ==================== app 后台按键 ====================

/// 常用键名 → VK 码。修饰键按左侧键处理；单字符键按大写 ASCII（字母）/ '0' 基（数字）。
fn vk_from_name(name: &str) -> Option<u32> {
    let normalized = name.trim().to_ascii_lowercase();
    let vk = match normalized.as_str() {
        "enter" | "return" => 0x0D,
        "esc" | "escape" => 0x1B,
        "tab" => 0x09,
        "space" | "spacebar" => 0x20,
        "backspace" => 0x08,
        "delete" | "del" => 0x2E,
        "insert" | "ins" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "up" | "arrowup" => 0x26,
        "down" | "arrowdown" => 0x28,
        "left" | "arrowleft" => 0x25,
        "right" | "arrowright" => 0x27,
        "ctrl" | "control" | "lctrl" | "leftcontrol" => 0x11,
        "shift" | "lshift" | "leftshift" => 0x10,
        "alt" | "lalt" | "leftalt" => 0x12,
        "win" | "meta" | "lwin" => 0x5B,
        "capslock" => 0x14,
        _ => {
            // F1~F12
            if let Some(rest) = normalized.strip_prefix('f') {
                if let Ok(n) = rest.parse::<u32>() {
                    if (1..=12).contains(&n) {
                        return Some(0x70 + n - 1);
                    }
                }
            }
            // 单字符键：字母/数字
            let chars: Vec<char> = normalized.chars().collect();
            if chars.len() == 1 {
                let c = chars[0].to_ascii_uppercase();
                if c.is_ascii_alphabetic() || c.is_ascii_digit() {
                    return Some(c as u32);
                }
            }
            return None;
        }
    };
    Some(vk)
}

/// 扩展键判定（方向键/Insert/Delete/Home/End/PgUp/PgDn，lParam 需置扩展位 0x01000000）。
fn is_extended_vk(vk: u32) -> bool {
    matches!(vk, 0x21..=0x28 | 0x2D | 0x2E)
}

/// 定位后台按键的投递目标：取目标窗口线程的内部焦点控件（GetGUIThreadInfo 对后台窗口同样有效）；
/// 拿不到内部焦点时退回窗口自身。
fn resolve_key_target_hwnd(hwnd: usize) -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO};

    let mut pid = 0u32;
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd as _, &mut pid) };
    if thread_id != 0 {
        let mut info: GUITHREADINFO = unsafe { core::mem::zeroed() };
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if unsafe { GetGUIThreadInfo(thread_id, &mut info) } != 0 && !info.hwndFocus.is_null() {
            return info.hwndFocus;
        }
    }
    hwnd as _
}

/// 后台按键：向目标窗口的内部焦点控件投递 WM_KEYDOWN/WM_KEYUP（不抢焦点、不改变真实键盘状态）。
/// lParam 携带扫描码与扩展标志，KeyUp 附加前态与切换位；修饰键按组合顺序先按下后逆序释放。
/// 全局快捷键/菜单加速键类应用可能不响应（这类处理在目标线程消息循环之外的加速键表）。
/// 返回投递方式："postmessage"。
pub fn app_key(hwnd: usize, keys: &[String], repeat: u32) -> Result<&'static str, String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VK_TO_VSC};
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, PostMessageW, WM_KEYDOWN, WM_KEYUP};

    if hwnd == 0 {
        return Err("目标窗口句柄为 0".to_string());
    }
    unsafe {
        if IsIconic(hwnd as _) != 0 {
            return Err("目标窗口已最小化，无法投递按键消息".to_string());
        }
    }
    if keys.is_empty() {
        return Err("按键组合为空".to_string());
    }

    let mut parsed = Vec::with_capacity(keys.len());
    for key in keys {
        let vk = vk_from_name(key).ok_or_else(|| format!("暂不支持后台按键 `{key}`"))?;
        let scan = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) } & 0xFF;
        parsed.push((vk, scan as isize));
    }

    let target = resolve_key_target_hwnd(hwnd);
    for _ in 0..repeat.max(1) {
        for (vk, scan) in &parsed {
            let mut lparam = (*scan) << 16; // 位 16~23 扫描码
            if is_extended_vk(*vk) {
                lparam |= 0x0100_0000; // 位 24 扩展键标志
            }
            unsafe {
                PostMessageW(target, WM_KEYDOWN, *vk as usize, lparam);
            }
        }
        for (vk, scan) in parsed.iter().rev() {
            let mut lparam = (*scan) << 16;
            if is_extended_vk(*vk) {
                lparam |= 0x0100_0000;
            }
            lparam |= 0xC000_0000; // 位 30 前态=1、位 31 切换=1
            unsafe {
                PostMessageW(target, WM_KEYUP, *vk as usize, lparam);
            }
        }
    }
    Ok("postmessage")
}

// ==================== 窗口枚举与激活 ====================

/// 全量枚举顶层可见窗口（含当前进程窗口；xcap 的 window_list 会过滤当前进程窗口，这里不用它）。
/// 返回按 z-order 排序（EnumWindows 顺序）的窗口信息列表。
pub fn list_all_windows() -> Vec<WindowInfo> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
        IsIconic, IsWindow, IsWindowVisible, EnumWindows, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    };
    use std::sync::Mutex;

    struct EnumCtx {
        windows: Vec<WindowInfo>,
        process_names: std::collections::HashMap<u32, String>,
    }
    let ctx = Mutex::new(EnumCtx { windows: Vec::new(), process_names: process_name_map() });
    let ctx_ptr = &ctx as *const Mutex<EnumCtx> as isize;

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> windows_sys::core::BOOL {
        let ctx = unsafe { &*(lparam as *const Mutex<EnumCtx>) };
        let mut guard = match ctx.lock() {
            Ok(g) => g,
            Err(_) => return 1,
        };
        // 只枚举顶层可见窗口；跳过工具窗口（WS_EX_TOOLWINDOW），保留无标题窗口（真实存在）
        if unsafe { IsWindow(hwnd) != 0 } && unsafe { IsWindowVisible(hwnd) != 0 } {
            let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
            if ex_style & WS_EX_TOOLWINDOW != 0 {
                return 1;
            }
            let title = read_window_title(hwnd);
            let mut pid = 0u32;
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut pid);
            }
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            unsafe {
                GetWindowRect(hwnd, &mut rect);
            }
            let minimized = unsafe { IsIconic(hwnd) != 0 };
            let fg = unsafe { GetForegroundWindow() };
            let process_name = guard.process_names.get(&pid).cloned();
            guard.windows.push(WindowInfo {
                window_id: hwnd as usize,
                title,
                process_name,
                process_id: pid,
                x: rect.left,
                y: rect.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
                minimized,
                focused: fg == hwnd,
            });
        }
        1
    }

    unsafe {
        EnumWindows(Some(enum_proc), ctx_ptr as isize);
    }
    match ctx.into_inner() {
        Ok(inner) => inner.windows,
        Err(_) => Vec::new(),
    }
}

/// 进程快照：pid → 可执行文件名（如 notepad.exe），供窗口枚举填充 process_name（G2 名字引用窗口）。
/// 快照失败返回空表，调用方降级为 None，不阻断枚举。
fn process_name_map() -> std::collections::HashMap<u32, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    let mut map = std::collections::HashMap::new();
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return map;
    }
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    if unsafe { Process32FirstW(snapshot, &mut entry) } != 0 {
        loop {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            if !name.is_empty() {
                map.insert(entry.th32ProcessID, name);
            }
            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }
    }
    unsafe {
        CloseHandle(snapshot);
    }
    map
}

fn read_window_title(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}

/// 激活窗口：还原最小化 → AttachThreadInput 绕前台锁 → SetForegroundWindow + BringWindowToTop → 轮询验证。
/// 返回 (窗口标题, 是否激活成功)。
///
/// 前台锁说明：SetForegroundWindow 默认只允许前台进程/收到最后输入事件的进程成功调用，
/// 后台进程直接调用会被系统静默拒绝。这里用 AttachThreadInput 把当前线程输入队列挂到
/// 目标窗口线程，使本进程获得设置前台窗口的资格（标准 API，无 UI 副作用）。
/// 不使用「模拟 Alt 键」的绕法：Alt 按下/释放会激活目标窗口菜单栏并显示 Key Tips，
/// 导致键盘焦点落在菜单栏而非正文控件，后续按键注入全部失效。
pub fn activate_window(window_id: usize) -> (String, bool) {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
        SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    let hwnd = window_id as *mut core::ffi::c_void;
    if hwnd.is_null() {
        return (String::new(), false);
    }
    let title = read_window_title(hwnd);
    unsafe {
        // 最小化则先还原
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        // 取目标窗口线程并挂接输入队列，绕前台锁（挂接失败不阻断，继续尝试激活）
        let mut target_pid = 0u32;
        let target_tid = GetWindowThreadProcessId(hwnd, &mut target_pid);
        let current_tid = GetCurrentThreadId();
        let attached = target_tid != 0
            && target_tid != current_tid
            && AttachThreadInput(current_tid, target_tid, 1) != 0;
        let _ = SetForegroundWindow(hwnd);
        let _ = BringWindowToTop(hwnd);
        if attached {
            AttachThreadInput(current_tid, target_tid, 0);
        }
        // 轮询验证（最多 1.5s）
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
        let mut activated = false;
        while std::time::Instant::now() < deadline {
            if GetForegroundWindow() == hwnd {
                activated = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        (title, activated)
    }
}

/// 窗口句柄是否仍然存在（含被隐藏的窗口）：用于区分「窗口已关闭」与「窗口被隐藏」。
pub fn window_is_alive(window_id: usize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;
    let hwnd = window_id as *mut core::ffi::c_void;
    if hwnd.is_null() {
        return false;
    }
    unsafe { IsWindow(hwnd) != 0 }
}

#[cfg(test)]
mod windows_platform_tests {
    use super::*;
    // tools.rs 被 include 进 main（crate root），window_list / virtual_desktop_bounds /
    // runtime_log_info 都在 crate root；eprintln! 宏在 main.rs:29 被重定义为 runtime_log_info
    use crate::runtime_log_info;

    #[test]
    fn control_type_whitelist_should_match_interactive_types() {
        assert!(is_interactive_control_type(UIA_ButtonControlTypeId));
        assert!(is_interactive_control_type(UIA_EditControlTypeId));
        assert!(is_interactive_control_type(UIA_CheckBoxControlTypeId));
        assert!(is_interactive_control_type(UIA_ComboBoxControlTypeId));
        assert!(is_interactive_control_type(UIA_ListItemControlTypeId));
        assert!(is_interactive_control_type(UIA_TabItemControlTypeId));
        assert!(is_interactive_control_type(UIA_MenuItemControlTypeId));
        assert!(is_interactive_control_type(UIA_HyperlinkControlTypeId));
        assert!(is_interactive_control_type(UIA_TreeItemControlTypeId));
        assert!(is_interactive_control_type(UIA_SliderControlTypeId));
        assert!(is_interactive_control_type(UIA_RadioButtonControlTypeId));
        assert!(is_interactive_control_type(UIA_DocumentControlTypeId));
    }

    #[test]
    fn non_interactive_types_should_be_rejected() {
        assert!(!is_interactive_control_type(UIA_CONTROLTYPE_ID(50001))); // unknown
        assert!(!is_interactive_control_type(UIA_CONTROLTYPE_ID(0)));
        assert!(!is_interactive_control_type(UIA_CustomControlTypeId));
    }

    #[test]
    fn normalize_element_name_should_trim_and_fold_whitespace() {
        // 实测 Chrome 元素名带尾空格 / 前导空格 / 内嵌换行（P2-4）
        assert_eq!(normalize_element_name("赞同 14 "), "赞同 14");
        assert_eq!(normalize_element_name(" 3 条评论"), "3 条评论");
        assert_eq!(normalize_element_name("Adblock Plus\n可以访问此网站"), "Adblock Plus 可以访问此网站");
        assert_eq!(normalize_element_name("  \t多  空白\n\n折行 "), "多 空白 折行");
        assert_eq!(normalize_element_name(""), "");
        // 归一后仍可做子串匹配：模型照抄展示名就能命中
        assert!(normalize_element_name("赞同 14 ").contains("赞同"));
    }

    #[test]
    fn candidate_groups_should_keep_page_elements_alongside_chrome() {
        // 扫描顺序是窗口框架在前、页面内容在后：分组后每类都能进候选（P0-2）
        let elements = vec![
            ("Button", "最小化"),
            ("Button", "恢复"),
            ("Button", "关闭"),
            ("Button", "写回答"),
            ("Edit", "地址和搜索栏"),
            ("Hyperlink", "赞同 14 "),
            ("Hyperlink", " 3 条评论"),
        ];
        let rendered = render_candidate_groups_from(elements.into_iter()).expect("分组非空");
        assert!(rendered.contains("Hyperlink：赞同 14、3 条评论"), "页面元素必须进候选：{rendered}");
        assert!(rendered.contains("Button：最小化、恢复、关闭"), "每类最多 3 个：{rendered}");
        assert!(!rendered.contains("写回答"), "同类超出 3 个应被截断：{rendered}");
    }

    #[test]
    fn candidate_groups_should_cap_type_count_and_report_remainder() {
        let elements = vec![
            ("Button", "a"),
            ("Edit", "b"),
            ("Hyperlink", "c"),
            ("ListItem", "d"),
            ("TabItem", "e"),
            ("MenuItem", "f"),
            ("Document", "g"),
        ];
        let rendered = render_candidate_groups_from(elements.into_iter()).expect("分组非空");
        // 最多 6 类，超出部分只报总数，不无限展开
        assert!(rendered.contains("共 7 类"), "超出类型数上限应报总数：{rendered}");
        assert_eq!(rendered.matches('：').count(), 6, "最多渲染 6 类：{rendered}");
    }

    #[test]
    fn candidate_groups_should_return_none_when_all_names_blank() {
        let elements = vec![("Button", ""), ("Edit", "   ")];
        assert!(render_candidate_groups_from(elements.into_iter()).is_none());
    }

    #[test]
    fn control_type_name_should_map_known_types() {
        assert_eq!(control_type_name(UIA_ButtonControlTypeId), "Button");
        assert_eq!(control_type_name(UIA_EditControlTypeId), "Edit");
        assert_eq!(control_type_name(UIA_DocumentControlTypeId), "Document");
        assert_eq!(control_type_name(UIA_PaneControlTypeId), "Pane");
        assert_eq!(control_type_name(UIA_TextControlTypeId), "Text");
        assert_eq!(control_type_name(UIA_CONTROLTYPE_ID(999)), "Unknown");
    }

    #[test]
    fn zero_inputs_should_return_empty() {
        assert!(collect_window_ui_elements(0, false).is_empty());
        assert!(collect_ui_tree_for_windows(&[], false).is_empty());
    }

    #[test]
    fn absolute_axis_should_map_virtual_desktop_span() {
        // 主屏 0~2559 映射到 0~65535
        assert_eq!(normalize_absolute_axis(0, 0, 2560), 0);
        assert_eq!(normalize_absolute_axis(2559, 0, 2560), 65535);
        assert_eq!(normalize_absolute_axis(1280, 0, 2560), 32780);
        // 副屏在主屏左侧：虚拟桌面原点 -1920、边长 4480，负坐标必须落在 0 以上
        assert_eq!(normalize_absolute_axis(-1920, -1920, 4480), 0);
        assert_eq!(normalize_absolute_axis(2559, -1920, 4480), 65535);
        assert!(normalize_absolute_axis(-800, -1920, 4480) > 0);
        assert!(normalize_absolute_axis(-800, -1920, 4480) < normalize_absolute_axis(0, -1920, 4480));
        // 边长非法时不 panic
        assert_eq!(normalize_absolute_axis(100, 0, 1), 0);
    }

    /// 真实桌面冒烟：绝对移动必须精确落在目标像素，测试结束把光标移回原位。
    /// 依赖真实 Windows 桌面，默认忽略，手动 `--ignored` 跑。
    /// 前台若是独占鼠标的窗口（全屏游戏用 Raw Input），注入会被系统接受但光标不动，
    /// 这种情况打印原因后按环境跳过，请在普通窗口前台重跑。
    #[test]
    #[ignore = "需要真实 Windows 桌面"]
    fn real_desktop_mouse_move_should_land_on_target_pixel() {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetCursorPos, GetForegroundWindow, GetSystemMetrics, GetWindowTextW, SM_CXVIRTUALSCREEN,
            SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        };

        // 与生产路径一致：operate 注入前会先声明 DPI 感知（operate_actions.rs），
        // 否则本进程看到的虚拟屏幕尺寸与系统注入用的物理尺寸不在同一空间
        let _ = enigo::set_dpi_awareness();
        let mut fg_title = [0u16; 256];
        let fg_len = unsafe { GetWindowTextW(GetForegroundWindow(), fg_title.as_mut_ptr(), fg_title.len() as i32) };
        let foreground = String::from_utf16_lossy(&fg_title[..fg_len.max(0) as usize]);
        unsafe {
            eprintln!(
                "[probe] foreground={foreground} virtual screen: {}x{} origin=({},{})",
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
            );
        }

        let mut origin = POINT { x: 0, y: 0 };
        assert_ne!(unsafe { GetCursorPos(&mut origin) }, 0, "读取光标位置失败");

        mouse_move_absolute(120, 140).expect("move mouse");
        let mut landed = POINT { x: 0, y: 0 };
        assert_ne!(unsafe { GetCursorPos(&mut landed) }, 0, "读取光标位置失败");
        eprintln!("[probe] landed=({}, {}) origin=({}, {})", landed.x, landed.y, origin.x, origin.y);
        if (landed.x, landed.y) == (origin.x, origin.y) {
            eprintln!("[probe] 光标未移动，前台 `{foreground}` 可能独占鼠标，本次跳过落点断言");
            return;
        }
        assert_eq!((landed.x, landed.y), (120, 140), "绝对移动落点与目标像素不一致");

        let _ = mouse_move_absolute(origin.x, origin.y);
    }

    /// 真实桌面冒烟测试：枚举窗口 + 激活前台窗口验证。
    /// 依赖真实 Windows 桌面，CI/无头环境不可用，默认忽略，手动 `--ignored` 跑。
    #[test]
    #[ignore = "需要真实 Windows 桌面"]
    fn real_desktop_should_list_and_activate_windows() {
        let windows = list_all_windows();
        assert!(!windows.is_empty(), "桌面应有可见窗口");
        eprintln!("[probe] total windows: {}", windows.len());
        for w in windows.iter().take(8) {
            eprintln!("[probe] id=0x{:x} title={:?} minimized={} focused={} rect=({},{},{},{})", w.window_id, w.title, w.minimized, w.focused, w.x, w.y, w.width, w.height);
        }
        // 激活当前前台窗口：应成功且标题一致
        if let Some(fg) = windows.iter().find(|w| w.focused) {
            let (title, activated) = activate_window(fg.window_id);
            eprintln!("[probe] activate focused window 0x{:x}: title={:?} activated={}", fg.window_id, title, activated);
            assert!(activated, "激活前台窗口应成功");
        }
    }

    /// 真实桌面冒烟测试：确认 UIA 客户端确实带着协议层超时（不是退回无超时的 v1）。
    /// 依赖真实 Windows 桌面，默认忽略，手动 `--ignored` 跑。
    #[test]
    #[ignore = "需要真实 Windows 桌面"]
    fn real_desktop_uia_client_should_carry_timeouts() {
        // 走 create_uia_automation 而不是裸 CoInitializeEx：COM 初始化与反初始化由 ComGuard 配平，
        // 测试自己初始化会让守卫无从收尾。
        let session = create_uia_automation().expect("create UIA");
        let automation2: IUIAutomation2 = session.0.cast().expect("应能取回 IUIAutomation2");
        unsafe {
            let connection = automation2.ConnectionTimeout().expect("读连接超时");
            let transaction = automation2.TransactionTimeout().expect("读事务超时");
            eprintln!("[probe] UIA timeouts: connection={connection}ms transaction={transaction}ms");
            assert_eq!(connection, UIA_CONNECTION_TIMEOUT_MS);
            assert_eq!(transaction, UIA_TRANSACTION_TIMEOUT_MS);
        }
    }

    /// 真实桌面冒烟测试：对本机主屏可见窗口扫一次控件树。
    /// 依赖真实 Windows 桌面，CI/无头环境不可用，默认忽略，手动 `--ignored` 跑。
    #[test]
    #[ignore = "需要真实 Windows 桌面"]
    fn real_desktop_should_scan_window_tree() {
        // window_list / virtual_desktop_bounds 是 crate root（main include）私有函数
        let windows = crate::window_list().expect("list windows");
        let targets: Vec<(usize, String)> = windows
            .iter()
            .take(3)
            .map(|w| (w.id().unwrap_or(0) as usize, w.title().unwrap_or_default()))
            .collect();
        assert!(!targets.is_empty(), "桌面应有可见窗口");
        let bounds = crate::virtual_desktop_bounds().expect("virtual desktop bounds");
        eprintln!("virtual desktop bounds: x={} y={} w={} h={}", bounds.x, bounds.y, bounds.width, bounds.height);
        let elems = collect_ui_tree_for_windows(&targets, false);
        // 不强制非空（自绘窗口/游戏可能不暴露 UIA），但扫描本身不能 panic
        eprintln!("scanned {} elements from {} windows", elems.len(), targets.len());
        for e in elems.iter().take(5) {
            eprintln!("  {}({}) at {}x{}", e.control_type, e.name, e.x, e.y);
        }
    }

    /// 记事本 UIA 树探针：拉起记事本，不做类型白名单过滤，打印全部元素的
    /// 控件类型 ID/名称，用于确认编辑区是否因白名单（如 Document 类型）被滤掉。
    /// 自拉自杀，Drop 守卫保证测试结束回收记事本进程。
    #[test]
    #[ignore = "需要真实 Windows 桌面"]
    fn probe_notepad_ui_tree_without_filter() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible};

        struct KillGuard(std::process::Child);
        impl Drop for KillGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        let child = std::process::Command::new("notepad").spawn().expect("launch notepad");
        let _guard = KillGuard(child);
        std::thread::sleep(std::time::Duration::from_millis(2000));

        static FOUND_HWNDS: std::sync::Mutex<Vec<isize>> = std::sync::Mutex::new(Vec::new());
        unsafe extern "system" fn enum_cb(hwnd: *mut core::ffi::c_void, _lparam: isize) -> i32 {
            if IsWindowVisible(hwnd) == 0 {
                return 1;
            }
            let len = GetWindowTextLengthW(hwnd);
            if len == 0 {
                return 1;
            }
            let mut buf = vec![0u16; len as usize + 1];
            GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            let title = String::from_utf16_lossy(&buf[..len as usize]);
            if title.contains("记事本") || title.to_lowercase().contains("notepad") {
                if let Ok(mut slot) = FOUND_HWNDS.lock() {
                    slot.push(hwnd as isize);
                }
            }
            1
        }

        let session = create_uia_automation().expect("UIA init");
        let automation = &session.0;
        unsafe {
            EnumWindows(Some(enum_cb), 0);
        }
        let hwnd = FOUND_HWNDS
            .lock()
            .unwrap()
            .first()
            .copied()
            .expect("notepad window not found");

        unsafe {
            let root = automation
                .ElementFromHandle(hwnd as *mut core::ffi::c_void)
                .expect("ElementFromHandle");
            let condition = automation.CreateTrueCondition().expect("condition");
            let array = root.FindAll(TreeScope_Descendants, &condition).expect("FindAll");
            let len = array.Length().unwrap_or(0);
            eprintln!("[notepad-probe] hwnd=0x{hwnd:x} total elements={len}");
            for i in 0..len {
                let Ok(elem) = array.GetElement(i) else { continue };
                let ct = elem.CurrentControlType().unwrap_or(UIA_CONTROLTYPE_ID(-1));
                let name = elem.CurrentName().ok().map(|b| b.to_string()).unwrap_or_default();
                let enabled = elem.CurrentIsEnabled().unwrap_or(false);
                let rect_str = elem
                    .CurrentBoundingRectangle()
                    .map(|r| format!("({},{},{},{})", r.left, r.top, r.right, r.bottom))
                    .unwrap_or_default();
                eprintln!(
                    "[notepad-probe] #{i} type={}({}) enabled={} name={:?} rect={}",
                    ct.0,
                    control_type_name(ct),
                    enabled,
                    name,
                    rect_str
                );
            }
        }
    }

    /// 记事本编辑区 pattern 支持面探针：验证 Document 编辑区是否支持
    /// ValuePattern 读回（getvalue 前提）与 ScrollPattern（后台滚动路径）。
    /// 自拉自杀，KillGuard 守卫。
    #[test]
    #[ignore = "需要真实 Windows 桌面"]
    fn probe_notepad_document_pattern_support() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible};

        struct KillGuard(std::process::Child);
        impl Drop for KillGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        let child = std::process::Command::new("notepad").spawn().expect("launch notepad");
        let _guard = KillGuard(child);
        std::thread::sleep(std::time::Duration::from_millis(2000));

        static FOUND_HWNDS: std::sync::Mutex<Vec<isize>> = std::sync::Mutex::new(Vec::new());
        unsafe extern "system" fn enum_cb(hwnd: *mut core::ffi::c_void, _lparam: isize) -> i32 {
            if IsWindowVisible(hwnd) == 0 {
                return 1;
            }
            let len = GetWindowTextLengthW(hwnd);
            if len == 0 {
                return 1;
            }
            let mut buf = vec![0u16; len as usize + 1];
            GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            let title = String::from_utf16_lossy(&buf[..len as usize]);
            if title.contains("记事本") || title.to_lowercase().contains("notepad") {
                if let Ok(mut slot) = FOUND_HWNDS.lock() {
                    slot.push(hwnd as isize);
                }
            }
            1
        }

        let session = create_uia_automation().expect("UIA init");
        let automation = &session.0;
        unsafe {
            EnumWindows(Some(enum_cb), 0);
        }
        let hwnd = FOUND_HWNDS
            .lock()
            .unwrap()
            .first()
            .copied()
            .expect("notepad window not found");

        unsafe {
            let root = automation
                .ElementFromHandle(hwnd as *mut core::ffi::c_void)
                .expect("ElementFromHandle");
            let condition = automation.CreateTrueCondition().expect("condition");
            let array = root.FindAll(TreeScope_Descendants, &condition).expect("FindAll");
            let len = array.Length().unwrap_or(0);

            let mut editor = None;
            for i in 0..len {
                if let Ok(elem) = array.GetElement(i) {
                    if elem.CurrentControlType().unwrap_or(UIA_CONTROLTYPE_ID(-1)) == UIA_DocumentControlTypeId {
                        editor = Some(elem);
                        break;
                    }
                }
            }
            let editor = editor.expect("Document editor element not found");

            // ValuePattern：读初始值 → SetValue 写入 → 再读回验证
            let vp_raw = editor
                .GetCurrentPatternRaw(UIA_VALUE_PATTERN_ID)
                .expect("query ValuePattern");
            if vp_raw.is_null() {
                eprintln!("[pattern-probe] ValuePattern: 不支持 → getvalue 需另寻路径");
            } else {
                let vp: IUIAutomationValuePattern = windows_core::Type::from_abi(vp_raw).expect("cast ValuePattern");
                let before = vp.CurrentValue().map(|b| b.to_string()).unwrap_or_else(|e| format!("读取失败:{e:?}"));
                eprintln!("[pattern-probe] ValuePattern: 支持，写前值={:?} len={}", before, before.chars().count());
                let bstr = windows_core::BSTR::from("后台探针写入-abc123");
                vp.SetValue(bstr.as_ptr() as *mut core::ffi::c_void).expect("SetValue");
                let after = vp.CurrentValue().map(|b| b.to_string()).unwrap_or_else(|e| format!("读取失败:{e:?}"));
                eprintln!("[pattern-probe] ValuePattern: 写后读回={:?} len={}", after, after.chars().count());
                assert_eq!(after, "后台探针写入-abc123", "写后读回不一致 → getvalue 语义存疑");
            }

            // ScrollPattern：只报告支持与否，不动真实滚动
            let sp_raw = editor
                .GetCurrentPatternRaw(UIA_SCROLL_PATTERN_ID)
                .expect("query ScrollPattern");
            eprintln!("[pattern-probe] ScrollPattern: {}", if sp_raw.is_null() { "不支持（走 PostMessage 兜底）" } else { "支持" });
        }
    }
}
