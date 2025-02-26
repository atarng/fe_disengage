#![feature(ptr_sub_ptr)]

use engage::{
    force::ForceType,
    gamedata::{unit::{ Unit, Gender }, skill::SkillData, PersonData},
    titlebar::TitleBar,
    gamesound::GameSound,
    mapmind::MapMind,
    menu::*,
    mess::*,
    proc::{desc::ProcDesc, Bindable, ProcInst},
    sequence::{ mapsequence::{ MapSequenceEngageConfirm, MapSequenceEngageSummon, human::MapSequenceHuman },
    mapsequencetargetselect::{MapSequenceTargetSelect, MapTarget} },
    util::{get_instance, get_singleton_proc_instance}
};

use mapunitcommand::{ MapUnitCommandMenu, MapUnitCommandMenuContent,
    TradeMenuItem, ItemMenuItem,
    EngageAttackMenuItem, EngageCommandMenuItem, EngageStartMenuItem, EngageSummonMenuItem, EngageLinkMenuItem, };
use mapsummon::{ MapSummonMenu, SummonColorMenuItem };

use skyline::{ install_hooks, patching::Patch, };
use std::sync::OnceLock;

use unity::{ prelude::*, system::List };

mod enume;
use enume::DisengageMapTargetEnumerator;

const LABEL_SUMMON_UI: u32  = 0x35; // 53
const LABEL_SUMMON_ACT: u32 = 0x36; // 54

const SUMMON_MIND_TYPE: u32 = 0x32;    // Decimal: 50
const DISENGAGE_MIND_TYPE: u32 = 0x38; // Decimal: 56
const REENGAGE_MIND_TYPE: u32 = 0x39;

impl Bindable for MapSequence { }
impl Bindable for MapSequenceHuman2 { }

#[unity::class("App", "MapBattleInfoRoot")]
pub struct MapBattleInfoRoot {
    sup: [u8;0x10],
    command_root: &'static (),
    command_sub_root: &'static (),
    command_text: &'static (),
    command_sub_text: &'static (),
    info_left: &'static (),
    info_right: &'static (),
}

#[unity::class("App", "MapSituation")]
pub struct MapSituation {
    sup: [u8;0x10],
    status: &'static (),
    players: &'static (),
    groups: &'static (),
    current_force_type: i32,    
}

impl MapSituation {
    pub fn get_target_unit(&self,  forcetype: i32)  -> i32 {
        unsafe { mapsituation_get_player(self, forcetype, None) }
    }
}

#[unity::class("App", "MapCursor")]
pub struct MapCursor {
    sup: [u8;0x10],
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
}

#[repr(C)]
#[unity::class("App", "MapSequence")]
pub struct MapSequence {
    pub descs: &'static mut Il2CppArray<&'static mut ProcDesc>,
    pub desc_index: i32,
    pub name: Option<&'static Il2CppString>,
    /// Unique ID derived from the name of the ProcInst.
    pub hashcode: i32,
    /// The ProcInst this instance is attached to
    pub parent: &'static mut ProcInst,
    /// The next ProcInst to process. ProcInsts are processed from child to parent.
    pub child: *mut MapSequenceHuman2,
}

#[repr(C)]
#[unity::class("App", "MapSequenceHuman")]
pub struct MapSequenceHuman2 {
    pub descs: &'static mut Il2CppArray<&'static mut ProcDesc>,
    pub desc_index: i32,
}

/// A structure representing a call to a method that returns nothing.
#[repr(C)]
#[unity::class("App", "ProcVoidMethod")]
pub struct ProcVoidMethodMut<T: 'static + Bindable> {
    method_ptr: *const u8,
    invoke_impl: *const u8,
    // Usually the ProcInst
    target: Option<&'static mut T>,
    // MethodInfo
    method: *const MethodInfo,
    __: [u8; 0x39],
    delegates: *const u8,
    // ...
}

impl<T: Bindable> engage::proc::Delegate for ProcVoidMethodMut<T> { }

impl<T: Bindable> ProcVoidMethodMut<T> {
    /// Prepare a ProcVoidMethod using your target and method of choice.
    /// Do be aware that despite the target argument being immutable, the receiving method can, in fact, mutate the target.
    pub fn new(
        target: impl Into<Option<&'static mut T>>,
        method: extern "C" fn(&'static mut T, OptionalMethod),
    ) -> &'static mut ProcVoidMethodMut<T> {
        ProcVoidMethodMut::<T>::instantiate().map(|proc| {
            proc.method_ptr = method as _;
            proc.target = target.into();
            proc.method = Box::leak(Box::new(MethodInfo::new())) as *mut MethodInfo;
            proc
        }).unwrap()
    }
}

// #[unity::class("App", "MapBattleInfoParamSetter")]
// pub struct MapBattleInfoParamSetter { }
// impl MapBattleInfoParamSetter {
//     pub fn set_battle_info_for_trade(&self) {
//         unsafe { mapbattleinfoparamsetter_setbattleinfofortrade(self, None) }
//     }
//     pub fn set_battle_info_for_no_param(&self, isweapon: bool, isgodname: bool) {
//         unsafe { mapbattleinfoparamsetter_setbattleinfofornoparam(self, isweapon, isgodname, None) }
//     }
// }
// #[unity::class("App", "SortieTradeItemMenuItem")]
// pub struct SortieTradeItemMenuItem {
//     sup: BasicMenuItemFields,
//     unit: Option<&'static mut Unit>,
//     receiver_unit: Option<&'static mut Unit>,
//     item_index: i32,
//     default_select: bool,
//     selectable_blank: bool,
//     enabled_to_select_blank: bool,
//     disabled: bool,
// }

#[unity::class("App", "InfoUtil")]
pub struct InfoUtil { }

impl InfoUtil {
    pub fn try_set_text(tmp: &(), string: impl Into<&'static Il2CppString>) {
        unsafe { infoutil_trysettext(tmp, string.into(), None) }
    }
}

#[unity::from_offset("App", "InfoUtil", "TrySetText")]
fn infoutil_trysettext(tmp: &(), str: &'static Il2CppString, method_info: OptionalMethod);


static DISENGAGE_CLASS: OnceLock<&'static mut Il2CppClass> = OnceLock::new();


#[skyline::main(name = "fe_disengage")]
pub fn main() {
    // Install a panic handler for your plugin, allowing you to customize what to do if there's an issue in your code.
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().unwrap();

        // Some magic thing to turn what was provided to the panic into a string. Don't mind it too much.
        // The message will be stored in the msg variable for you to use.
        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => {
                match info.payload().downcast_ref::<String>() {
                    Some(s) => &s[..],
                    None => "Box<Any>",
                }
            },
        };

        // This creates a new String with a message of your choice, writing the location of the panic and its message inside of it.
        // Note the \0 at the end. This is needed because show_error is a C function and expects a C string.
        // This is actually just a result of bad old code and shouldn't be necessary most of the time.
        let err_msg = format!(
            "fe_disengage has panicked at '{}' with the following message:\n{}\0",
            location,
            msg
        );

        // We call the native Error dialog of the Nintendo Switch with this convenient method.
        // The error code is set to 69 because we do need a value, while the first message displays in the popup and the second shows up when pressing Details.
        skyline::error::show_error(
            69,
            "fe_disengage has panicked! Please open the details and send a screenshot to the developer, then close the game.\n\0",
            err_msg.as_str(),
        );
    }));

    install_hooks!(
        mapunitcommandmenu_createbind,
        maptarget_enumerate,
        mapbattleinforoot_setcommandtext,
        ///////////////////////////
        mapsequencetargetselect_decide,
        mapsequencetargetselect_decide_normal,
        mapsequencetargetselect_trychangeengage,
        maptarget_enumerateengagelink,
        mapsequencehuman_createbind,
    );
    install_hooks!(
        engageattackmenuitem_ctor,
        engagestartmenuitem_ctor,
        engagesummonmenuitem_ctor,
        engagelinkmenuitem_ctor,
        mapsequenceengageconfirm_ctor,
        mapsequenceengagesummon_ctor,
        mapsummonmenu_ctor,
        summoncolormenuitem_ctor,
        /////////////////////
        mapsummonmenu_createsummonbind,
        engagecommandmenuitem_acall,
        engagelinkmenuitem_acall,
        summoncolormenuitem_acall,
        ////////////////////
        mapsequenceengagesummon_mindstart_hook,
        mapsequenceengagesummon_mindend_hook,
        mapsequenceengagesummon_combatsummon,
        mapsequenceengagesummon_simplesummon_hook,
        mapsequenceengagesummon_commit_hook,
        ///////////////////////////
        mapsequenceengagesummon_createbind_hook,
        mapsequenceengagesummon_createtelop_hook,
        mapsequenceengagesummon_setperson_hook,
        mapsequenceengagesummon_calculate_hook,
    );
}

// Create our new menu command for Disengage
#[unity::hook("App", "MapUnitCommandMenu", "CreateBind")]
pub fn mapunitcommandmenu_createbind(sup: &mut ProcInst, _method_info: OptionalMethod) {
    println!("[mapunitcommandmenu_createbind] BEG");

    // let maptarget_instance = get_instance::<MapTarget>();
    // let cur_mind = maptarget_instance.m_mind;
    // println!("[mapunitcommandmenu_createbind] cur_mind: {}", cur_mind);

    //// 0x7101e518f0
    //// void App.MapUnitCommandMenu.TradeMenuItem$$.ctor(App_MapUnitCommandMenu_TradeMenuItem_o *__this,MethodInfo *method)
    //// Create a new class using TradeMenuItem as reference so that we do not wreck the original command for ours.
    // Create a new class using EngageSummonMenuItem as reference so that we do not wreck the original command for ours.
    let disengage = DISENGAGE_CLASS.get_or_init(|| {
        // EngageSummonMenuItem is a nested class inside of MapUnitCommandMenu, so we need to dig for it.
        // let menu_class  = *MapUnitCommandMenu::class()
        //     .get_nested_types()
        //     .iter()
        //     .find(|class| class.get_name().contains("TradeMenuItem"))
        //     .unwrap();
        ///////////////////////////////////////////////////////////////////////
        //// This one doesn't show up at all.
        // let menu_class  = *MapUnitCommandMenu::class()
        //     .get_nested_types()
        //     .iter()
        //     .find(|class| class.get_name().contains("EngageSummonMenuItem"))
        //     .unwrap();
        ////////////////////////////////////////////////////////////////////////
        // This just enters engage mode.
        let menu_class  = *MapUnitCommandMenu::class()
            .get_nested_types()
            .iter()
            .find(|class| class.get_name().contains("EngageStartMenuItem"))
            .unwrap();

        //////////////////////////////////////////////////////////
        // .... Repositions?
        // let menu_class  = *MapSummonMenu::class()
        //     .get_nested_types()
        //     .iter()
        //     .find(|class| class.get_name().contains("SummonColorMenuItem"))
        //     .unwrap();

        let new_class = menu_class.clone();
        new_class
            .get_virtual_method_mut("GetName")
            .map(|method| method.method_ptr = disengage_get_name as _)
            .unwrap();
        new_class
            .get_virtual_method_mut("GetCommandHelp")
            .map(|method| method.method_ptr = disengage_get_desc as _)
            .unwrap();
        new_class
            .get_virtual_method_mut("get_ActiveMind")
            .map(|method| method.method_ptr = disengage_get_active_mind as _)
            .unwrap();
        new_class
            .get_virtual_method_mut("get_Mind")
            .map(|method| method.method_ptr = disengage_get_mind as _)
            .unwrap();
        // public const MapPanelDeploy.Mode UnitCommand = 10;
        // int32_t App.MapUnitCommandMenu.EngageCommandMenuItem$$get_DeployMode(App_MapUnitCommandMenu_EngageCommandMenuItem_o *__this,MethodInfo *method)
        new_class
             .get_virtual_method_mut("get_FlagID")
             .map(|method| method.method_ptr = disengage_get_flagid as _)
             .unwrap();


        new_class
    });

    call_original!(sup, _method_info);
    
    // Instantiate our custom class as if it was EngageSummonMenuItem
    // let instance = Il2CppObject::<TradeMenuItem>::from_class(disengage).unwrap();
    // let menu_item_list = &mut sup.child.as_mut().unwrap().cast_mut::<BasicMenu<TradeMenuItem>>().full_menu_item_list;
    /////////////////////////////////////
    // let instance = Il2CppObject::<EngageSummonMenuItem>::from_class(disengage).unwrap();
    // let menu_item_list = &mut sup.child.as_mut().unwrap().cast_mut::<BasicMenu<EngageSummonMenuItem>>().full_menu_item_list;
    ////////////////////////////////////
    let instance = Il2CppObject::<EngageStartMenuItem>::from_class(disengage).unwrap();
    let menu_item_list = &mut sup.child.as_mut().unwrap().cast_mut::<BasicMenu<EngageStartMenuItem>>().full_menu_item_list;
    ////////////////////////////////////
    // let instance = Il2CppObject::<SummonColorMenuItem>::from_class(disengage).unwrap();
    // let menu_item_list = &mut sup.child.as_mut().unwrap().cast_mut::<BasicMenu<SummonColorMenuItem>>().full_menu_item_list;

    menu_item_list.insert((menu_item_list.len() - 1) as i32, instance);

    println!("[mapunitcommandmenu_createbind] END");
}

// MapMind.Type Talk = 2;          // PA 
// MapMind.Type Attack = 3;        // PA
// MapMind.Type EngageLink = 5;
// MapMind.Type Rod = 12;          // PA
// MapMind.Type Destroy = 11;      // PA
// MapMind.Type Trade = 15;        // PA
// MapMind.Type Dance = 34;        // PA
// MapMind.Type CommandSkill = 38;
// MapMind.Type EngageSummon = 50;
// MapMind.Type Contract = 54;

/////////////////
// 0x7101f32d90
// #[skyline::hook(offset=0x01f32d90)]
// This is a generic function that essentially checks the Mind value, and then calls
// a more specialized Enumerate function based on the result.
// Enumerate functions are used for checking if there is a valid target in range,
// and making a list of them.
#[unity::hook("App", "MapTarget", "Enumerate")]
pub fn maptarget_enumerate(this: &mut MapTarget, mask: i32, _method_info: OptionalMethod) {
    println!("[maptarget_enumerate] BEG: mind: {} mask: {}", this.m_mind, mask);
    // issues matching between u32 and i32?
    // match this.m_mind {
    if this.m_mind == DISENGAGE_MIND_TYPE {
        println!("[maptarget_enumerate] disengage");

        // this.m_action_mask = mask as u32;
        // if let Some(unit) = this.unit {
        //     if this.x < 0 { this.x = unit.x as i8; }
        //     if this.z < 0 { this.z = unit.z as i8; }
        // }
        // if let Some(dataset) = this.m_dataset.as_mut() {
        //     dataset.clear();
        // }
        // // this.enumerate_disengage();
        // ///////////////////////////////////
        // this.enumerate_self_only(mask);
        // if let Some(dataset) = this.m_dataset.as_mut() {
        //     dataset.m_list
        //         .iter_mut()
        //         .enumerate()
        //         .for_each(|(count_var, data_item)| {
        //             data_item.m_index = count_var as i8;    
        //         });
        // }

        // 0x35: 53
        // This is using the new label added in the mapsequencehuman_createbind.
        this.enumerate_self_only(mask);
        let mapsequencehuman_instance = get_singleton_proc_instance::<MapSequenceHuman>().unwrap();
        ProcInst::jump(mapsequencehuman_instance, LABEL_SUMMON_UI.try_into().unwrap());

    } else if this.m_mind == REENGAGE_MIND_TYPE {
        println!("[maptarget_enumerate] reengage");
        this.m_action_mask = mask as u32;
        if let Some(unit) = this.unit {
            if this.x < 0 {
                this.x = unit.x as i8;
            }
            if this.z < 0 {
                this.z = unit.z as i8;
            }
        }
        if let Some(dataset) = this.m_dataset.as_mut() {
            dataset.clear();
        }
        this.enumerate_reengage();
        if let Some(dataset) = this.m_dataset.as_mut() {
            dataset.m_list
                .iter_mut()
                .enumerate()
                .for_each(|(count_var, data_item)| {
                    data_item.m_index = count_var as i8;    
                });
        }
    } else {
        println!("[maptarget_enumerate] mind: {}", this.m_mind);
        call_original!(this, mask, _method_info);
    }
    println!("[maptarget_enumerate] END");
}

// Make "Steal" appear on the preview when highlighting an enemy to steal from.
// This function is what sets the text that appears in between the two windows
// when highlighting an enemy.
#[unity::hook("App", "MapBattleInfoRoot", "SetCommandText")]
pub fn mapbattleinforoot_setcommandtext(this: &mut MapBattleInfoRoot, mind_type: i32, _method_info: OptionalMethod) {
    println!("[mapbattleinforoot_setcommandtext/BEG]");
    if DISENGAGE_MIND_TYPE == mind_type.try_into().unwrap() {
        InfoUtil::try_set_text(&this.command_text, "Disengage[SCT]");
    } else if REENGAGE_MIND_TYPE == mind_type.try_into().unwrap() {
        InfoUtil::try_set_text(&this.command_text, "Reengage[SCT]");
    } else {
        call_original!(this, mind_type, _method_info);
    }
    println!("[mapbattleinforoot_setcommandtext/END]");
}

// 0x7101e4e310
// void App.MapUnitCommandMenu.EngageStartMenuItem$$.ctor(App_MapUnitCommandMenu_EngageStartMenuItem_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x01e4e310)]
pub fn engagestartmenuitem_ctor(this: &EngageStartMenuItem, _method_info: OptionalMethod) {
    println!("[engagestartmenuitem_ctor] BEG");
    call_original!(this, _method_info);
    println!("[engagestartmenuitem_ctor] END");
}

// 0x7101e4d670
// void App.MapUnitCommandMenu.EngageAttackMenuItem$$.ctor(App_MapUnitCommandMenu_EngageSummonMenuItem_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x01e4d670)]
pub fn engageattackmenuitem_ctor(this: &EngageAttackMenuItem, _method_info: OptionalMethod) {
    println!("[engageattackmenuitem_ctor] BEG");
    call_original!(this, _method_info);
    println!("[engageattackmenuitem_ctor] END");
}

// 0x7101e4d7c0
// int32_t App.MapUnitCommandMenu.EngageCommandMenuItem$$ACall(App_MapUnitCommandMenu_EngageCommandMenuItem_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x01e4d7c0)]
pub fn engagecommandmenuitem_acall(this: &EngageCommandMenuItem, _method_info: OptionalMethod) -> i32 {
    println!("[engagecommandmenuitem_acall/BEG]");
    let original = call_original!(this, _method_info);
    println!("[engagecommandmenuitem_acall/END] original: {}", original);
    return original;
}

// 0x7101e4dcd0
// int32_t App.MapUnitCommandMenu.EngageLinkMenuItem$$ACall(App_MapUnitCommandMenu_EngageLinkMenuItem_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x01e4dcd0)]
pub fn engagelinkmenuitem_acall(this: &EngageLinkMenuItem, _method_info: OptionalMethod) -> i32 {
    println!("[engagelinkmenuitem_acall/BEG]");
    let original = call_original!(this, _method_info);
    println!("[engagelinkmenuitem_acall/END] original: {}", original);
    return original;
}

// 0x7101e4e380
// void App.MapUnitCommandMenu.EngageSummonMenuItem$$.ctor(App_MapUnitCommandMenu_EngageSummonMenuItem_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x01e4e380)]
pub fn engagesummonmenuitem_ctor(this: &EngageSummonMenuItem, _method_info: OptionalMethod) {
    println!("[engagesummonmenuitem_ctor/BEG]");
    call_original!(this, _method_info);
    println!("[engagesummonmenuitem_ctor/END]");
}

// 0x7101e4dd90
// void App.MapUnitCommandMenu.EngageLinkMenuItem$$.ctor(App_MapUnitCommandMenu_EngageLinkMenuItem_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x01e4dd90)]
pub fn engagelinkmenuitem_ctor(this: &EngageLinkMenuItem, _method_info: OptionalMethod) {
    println!("[engagelinkmenuitem_ctor/BEG]");
    call_original!(this, _method_info);
    println!("[engagelinkmenuitem_ctor/END]");
}

// 0x71023ce870
// void App.MapSequenceEngageConfirm$$.ctor(App_MapSequenceEngageConfirm_o *__this,MethodInfo *method)
// Doesn't seem to be triggered from Engaging
#[skyline::hook(offset = 0x023ce870)]
pub fn mapsequenceengageconfirm_ctor(this: &MapSequenceEngageConfirm, _method_info: OptionalMethod) {
    println!("[mapsequenceengageconfirm_ctor/BEG]");
    call_original!(this, _method_info);
    println!("[mapsequenceengageconfirm_ctor/END]");
}

// This is each summon color menu item.
// 0x7101e3eda0
// void App.MapSummonMenu.SummonColorMenuItem$$.ctor(App_MapSummonMenu_SummonColorMenuItem_o *__this,int32_t color,MethodInfo *method)
#[skyline::hook(offset = 0x01e3eda0)]
pub fn summoncolormenuitem_ctor(this: &SummonColorMenuItem, color: i32, _method_info: OptionalMethod) {
    println!("[summoncolormenuitem_ctor/BEG] color: {}", color);
    call_original!(this, color, _method_info);
    println!("[summoncolormenuitem_ctor/END]");
}

// 0x71023cf390
// void App.MapSequenceEngageSummon$$.ctor(App_MapSequenceEngageSummon_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x023cf390)]
pub fn mapsequenceengagesummon_ctor(this: &mut MapSequenceEngageSummon, _method_info: OptionalMethod) {
    println!("[mapsequenceengagesummon_ctor/BEG]");
    call_original!(this, _method_info);
    println!("[mapsequenceengagesummon_ctor/END]");
}

///////////////////////////////////////////////
//// Non-intrusive way of overriding a function? ////

pub extern "C" fn disengage_get_name(_this: &(), _method_info: OptionalMethod) -> &'static Il2CppString {
    "Separate".into()
}

pub extern "C" fn disengage_get_desc(_this: &(), _method_info: OptionalMethod) -> &'static Il2CppString {
    "Summon equipped emblem to fight with you.".into()
}

// what does 0x38 mean??? Decimal: 56 binary: 0b111000
// dump.cs: public enum MapMind.Type // TypeDefIndex: 12219
pub extern "C" fn disengage_get_mind(_this: &(), _method_info: OptionalMethod) -> i32 {
    return DISENGAGE_MIND_TYPE.try_into().unwrap();
}

pub extern "C" fn disengage_get_flagid(_this: &(), _method_info: OptionalMethod) -> &'static Il2CppString {
    "Separate".into()
}

pub extern "C" fn disengage_get_active_mind(_this: &(), _method_info: OptionalMethod) -> i32 {
    return SUMMON_MIND_TYPE.try_into().unwrap();
}

///////////////////////////////////////////////

#[unity::from_offset("App", "MapSituation", "GetPlayer")]
fn mapsituation_get_player(this: &MapSituation, forcetype: i32, method_info: OptionalMethod) -> i32;

// Functions for the ProcDesc fuckening
#[unity::from_offset("App", "MapSequenceHuman", "UnitMenuPrepare")]
fn mapsequencehuman_unitmenuprepare(this: &MapSequenceHuman, method_info: OptionalMethod);
#[unity::from_offset("App", "MapSequenceHuman", "PreItemMenuTrade")]
fn mapsequencehuman_preitemmenutrade(this: &MapSequenceHuman, method_info: OptionalMethod);
#[unity::from_offset("App", "MapItemMenu", "CreateBindTrade")]
fn mapitemmenu_createbindtrade(sup: &ProcInst, method_info: OptionalMethod);
#[unity::from_offset("App", "MapSequenceHuman", "PostItemMenuTrade")]
fn mapsequencehuman_postitemmenutrade(this: &MapSequenceHuman, method_info: OptionalMethod);

// This function is... interesting.  It essentially builds a BIG list of labels and functions to run.
// The labels are a way for the game to jump around the list and then run a series of functions in a row.
// This is essentially how the ENTIRE game functions to some degree.
// What we're doing here is adding a new section of entries to the list specifically for the Steal command.
// We insert the new function calls and labels in reverse order because adding something to an existing index
// pushes whatever was already there forward, and also makes later additions simpler.
// ===========================================================================================
// This appears to get called once at the very start.
// 0x7102677780
#[skyline::hook(offset = 0x2677780)]
pub fn mapsequencehuman_createbind(sup: &mut MapSequence, is_resume: bool, _method_info: OptionalMethod) {
    println!("[mapsequencehuman_createbind] BEG");
    call_original!(sup, is_resume, _method_info);

    let mut vec = unsafe { (*(sup.child)).descs.to_vec() };

    //////////////////////////////////////////////////////
    
    // 0x10: 16::
    //// OPEN SUMMON MENU
    //// UnitCommand: 16 :: 0x10
    // let desc = engage::proc::desc::ProcDesc::jump(0x10);
    // EngageSummonMenu = 43 :: 0x2b
    let desc = engage::proc::desc::ProcDesc::jump(0x2b);
    vec.insert(0x9a, desc);

    let method = mapsummonmenu_createbindsummon::get_ref();
    let method = unsafe { std::mem::transmute(method.method_ptr) };
    let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    vec.insert(0x9a, desc);
    //////////////////////
    let method = mapsequencehuman_unitmenuprepare::get_ref();
    let method = unsafe { std::mem::transmute(method.method_ptr) };
    let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    vec.insert(0x9a, desc);

    // public enum MapSequenceHuman.Label: entries, 53 is new. (0x35) LABEL_SUMMON_UI
    let summon_ui_label = ProcDesc::label(53);
    vec.insert(0x9a, summon_ui_label);

    ////////////////////////////////////////////////
    // ItemMenuEngageAttack:: 19 :: 0x13
    // let desc = engage::proc::desc::ProcDesc::jump(0x13);
    // UnitCommand = 16;
    let desc = engage::proc::desc::ProcDesc::jump(0x10);
    vec.insert(0x9a, desc);
    
    // let method = mapsequenceengagesummon_mindend::get_ref();
    // let method = unsafe { std::mem::transmute(method.method_ptr) };
    // let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    // vec.insert(0x9a, desc);
    // let method = mapsequenceengagesummon_simplesummon::get_ref();
    // let method = unsafe { std::mem::transmute(method.method_ptr) };
    // let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    // vec.insert(0x9a, desc);
    /////////////////////////////////////
    let method = mapsequenceengagesummon_commit::get_ref();
    let method = unsafe { std::mem::transmute(method.method_ptr) };
    let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    vec.insert(0x9a, desc);
    // let method = mapsequenceengagesummon_mindstart::get_ref();
    // let method = unsafe { std::mem::transmute(method.method_ptr) };
    // let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    // vec.insert(0x9a, desc);
    let method = mapsequenceengagesummon_createbind::get_ref();
    let method = unsafe { std::mem::transmute(method.method_ptr) };
    let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    vec.insert(0x9a, desc);

    // LABEL_SUMMON_ACT
    let summon_act_label = ProcDesc::label(54);
    vec.insert(0x9a, summon_act_label);

    ////////////////////////////////////////////////////////////////////////

    // let mut vec = unsafe { (*(sup.child)).descs.to_vec() };
    // let desc = engage::proc::desc::ProcDesc::jump(0x10);
    // vec.insert(0x9a, desc);
    // let method = mapsequencehuman_postitemmenutrade::get_ref();
    // let method = unsafe { std::mem::transmute(method.method_ptr) };
    // let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    // vec.insert(0x9a, desc);
    //////////////
    // let method = mapitemmenu_createbindtrade::get_ref();
    // let method = unsafe { std::mem::transmute(method.method_ptr) };
    // let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    // vec.insert(0x9a, desc);
    //////////////
    // let method = mapsequencehuman_preitemmenutrade::get_ref();
    // let method = unsafe { std::mem::transmute(method.method_ptr) };
    // let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    // vec.insert(0x9a, desc);
    //////////////
    // let method = mapsequencehuman_unitmenuprepare::get_ref();
    // let method = unsafe { std::mem::transmute(method.method_ptr) };
    // let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    // vec.insert(0x9a, desc);
    ///////////////////////////////////////////////////////////////////

    let new_descs = Il2CppArray::from_slice(vec).unwrap();
    unsafe { (*sup.child).descs = new_descs };
    println!("[mapsequencehuman_createbind] END");
}

// 0x7102676fb0
// void App.MapSequenceHuman$$EngageBeforeEvent(App_MapSequenceHuman_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x02676fb0)]
pub fn mapsequencehuman_engagebeforeevent(this: &mut MapSequenceHuman, _method_info: OptionalMethod) {
    println!("[mapsequencehuman_engagebeforeevent/BEG]");
    call_original!(this, _method_info);
    println!("[mapsequencehuman_engagebeforeevent/END]");
}

// This is the function that usually runs when you press A while highlighting a target and the forecast windows are up.
#[unity::hook("App", "MapSequenceTargetSelect", "Decide")]
pub fn mapsequencetargetselect_decide(this: &mut MapSequenceTargetSelect, _method_info: OptionalMethod) {
    println!("[mapsequencetargetselect_decide/BEG]");
    let maptarget_instance = get_instance::<MapTarget>();
    let cur_mind = maptarget_instance.m_mind;
    if cur_mind == DISENGAGE_MIND_TYPE {
        println!("[mapsequencetargetselect_decide] DISENGAGE ({})", cur_mind);
        // let mapsequencehuman_instance = get_singleton_proc_instance::<MapSequenceHuman>().unwrap();
        // // This is using the new label added in the mapsequencehuman_createbind.
        // // 0x35: 53
        // ProcInst::jump(mapsequencehuman_instance, 0x35);
        
        // 0x32 : 50
        // maptarget_instance.m_mind = 0x32;

        // maptarget_enumerate
        // maptarget_instance.m_action_mask = mask as u32;
        // if let Some(unit) = maptarget_instance.unit {
        //     if maptarget_instance.x < 0 { maptarget_instance.x = unit.x as i8; }
        //     if maptarget_instance.z < 0 { maptarget_instance.z = unit.z as i8; }
        // }
        // if let Some(dataset) = maptarget_instance.m_dataset.as_mut() {
        //     dataset.clear();
        // }
        // // ///////////////////////////////////
        // this.enumerate_self_only(mask);
        // if let Some(dataset) = maptarget_instance.m_dataset.as_mut() {
        //     dataset.m_list
        //         .iter_mut()
        //         .enumerate()
        //         .for_each(|(count_var, data_item)| {
        //             data_item.m_index = count_var as i8;    
        //         });
        // }

        call_original!(this, _method_info);

        GameSound::post_event("Decide", None);
    } else {
        println!("[mapsequencetargetselect_decide] mind: {}", cur_mind);
        call_original!(this, _method_info);
    }
    println!("[mapsequencetargetselect_decide/END]");
}

// This is the function that usually runs when you press A while highlighting a target and the forecast windows are up.
#[unity::hook("App", "MapSequenceTargetSelect", "DecideNormal")]
pub fn mapsequencetargetselect_decide_normal(this: &mut MapSequenceTargetSelect, _method_info: OptionalMethod) {
    println!("[mapsequencetargetselect_decide_normal/BEG]");

    let maptarget_instance = get_instance::<MapTarget>();
    let cur_mind = maptarget_instance.m_mind;
    if cur_mind == DISENGAGE_MIND_TYPE {
        println!("[mapsequencetargetselect_decide_normal] STEAL/DISENGAGE ({})", cur_mind);
        let mapmind_instance = get_instance::<MapMind>();
        //// Start off empty I think? although this doesn't look like force.
        // let mut unit_index = 7;
        // if this.can_select_target() && this.target_data.is_some() {
        //     unit_index = this.target_data.unwrap().m_unit.index;
        // }
        // mapmind_instance.set_trade_unit_index(unit_index as _);
        /////////////////////////////////////////
        // mapmind_instance.set_unit()


        let mapsequencehuman_instance = get_singleton_proc_instance::<MapSequenceHuman>().unwrap();
        // This is using the new label added in the mapsequencehuman_createbind.
        // 0x36: 54
        ProcInst::jump(mapsequencehuman_instance, LABEL_SUMMON_ACT.try_into().unwrap());
        GameSound::post_event("Decide", None);
    } else {
        println!("[mapsequencetargetselect_decide_normal] mind: {}", cur_mind);
        call_original!(this, _method_info);
    }

    /////////////////////////////////////////////////////
    // call_original!(this, _method_info);
    println!("[mapsequencetargetselect_decide_normal/END]");
}

// 0x7101f37e20
// bool App.MapSequenceTargetSelect$$TryChangeEngage(App_MapSequenceTargetSelect_o *__this,MethodInfo *method)
#[unity::hook("App", "MapSequenceTargetSelect", "TryChangeEngage")]
pub fn mapsequencetargetselect_trychangeengage(this: &mut MapSequenceTargetSelect, _method_info: OptionalMethod) -> bool {
    println!("[mapsequencetargetselect_trychangeengage/BEG]");
    let mut original = call_original!(this, _method_info);
    println!("[mapsequencetargetselect_trychangeengage/END] original: {}", original);
    return original;
}

// 0x7101f58980
// void App.MapTarget$$EnumerateEngageLink(App_MapTarget_o *__this,MethodInfo *method)
#[unity::hook("App", "MapTarget", "EnumerateEngageLink")]
pub fn maptarget_enumerateengagelink(this: &mut MapTarget, _method_info: OptionalMethod) {
    println!("[maptarget_enumerateengagelink/BEG]");
    call_original!(this, _method_info);
    println!("[maptarget_enumerateengagelink/END]");
}

/////////////////////////////////////////////////////////////////////

#[unity::from_offset("App", "MapSummonMenu", "CreateSummonBind")]
fn mapsummonmenu_createbindsummon(supper: &ProcInst, method_info: OptionalMethod);

// 0x7101f51dd0
// void App.MapSummonMenu$$CreateSummonBind(App_ProcInst_o *super,MethodInfo *method)
#[skyline::hook(offset = 0x01f51dd0)]
pub fn mapsummonmenu_createsummonbind(supper: &ProcInst, _method_info: OptionalMethod) {
    println!("[mapsummonmenu_createsummonbind/BEG]");
    call_original!(supper, _method_info);
    println!("[mapsummonmenu_createsummonbind/END]");
}

// 0x7101f51dd0
// void App.MapSummonMenu$$.ctor(App_MapSummonMenu_o *__this, System_Collections_Generic_List_BasicMenuItem__o *menuItemList, App_MapUnitCommandMenuContent_o *menuContent,MethodInfo *method)
#[skyline::hook(offset = 0x01f51dd0)]
pub fn mapsummonmenu_ctor(this: &MapSummonMenu, menu_item_list: Option<&List<BasicMenuItem>>, menu_content: Option<&MapUnitCommandMenuContent>, _method_info: OptionalMethod) {
    println!("[mapsummonmenu_ctor/BEG]");
    call_original!(this, menu_item_list, menu_content, _method_info);
    println!("[mapsummonmenu_ctor/END]");
}

// 0x7101e3f0a0
// int32_t App.MapSummonMenu.SummonColorMenuItem$$ACall(App_MapSummonMenu_SummonColorMenuItem_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x01e3f0a0)]
pub fn summoncolormenuitem_acall(this: &SummonColorMenuItem, _method_info: OptionalMethod) -> i32 {
    println!("[summoncolormenuitem_acall/BEG]");
    let original = call_original!(this, _method_info);
    println!("[summoncolormenuitem_acall/END] original: {}", original);
    return original;
}

// 0x71023cf3f0
// void App.MapSequenceEngageSummon$$MindStart(App_MapSequenceEngageSummon_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x023cf3f0)]
pub fn mapsequenceengagesummon_mindstart_hook(this: &MapSequenceEngageSummon, _method_info: OptionalMethod) {
    println!("[mapsequenceengagesummon_mindstart_hook/BEG]");
    call_original!(this, _method_info);
    println!("[mapsequenceengagesummon_mindstart_hook/END]");
}

// 0x71023cf4a0
// void App.MapSequenceEngageSummon$$MindEnd(App_MapSequenceEngageSummon_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x023cf4a0)]
pub fn mapsequenceengagesummon_mindend_hook(this: &MapSequenceEngageSummon, _method_info: OptionalMethod) {
    println!("[mapsequenceengagesummon_mindend_hook/BEG]");
    call_original!(this, _method_info);
    println!("[mapsequenceengagesummon_mindend_hook/END]");
}

// 0x71023cfa40
// void App.MapSequenceEngageSummon$$CombatSummon(App_MapSequenceEngageSummon_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x023cfa40)]
pub fn mapsequenceengagesummon_combatsummon(this: &MapSequenceEngageSummon, _method_info: OptionalMethod) {
    println!("[mapsequenceengagesummon_combatsummon/BEG]");
    call_original!(this, _method_info);
    println!("[mapsequenceengagesummon_combatsummon/END]");
}

// 0x71023cf940
// void App.MapSequenceEngageSummon$$SimpleSummon(App_MapSequenceEngageSummon_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x023cf940)]
pub fn mapsequenceengagesummon_simplesummon_hook(this: &MapSequenceEngageSummon, _method_info: OptionalMethod) {
    println!("[mapsequenceengagesummon_simplesummon_hook/BEG]");
    call_original!(this, _method_info);
    println!("[mapsequenceengagesummon_simplesummon_hook/END]");
}

// 0x71023cf940
// void App.MapSequenceEngageSummon$$Commit(App_MapSequenceEngageSummon_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x023cf940)]
pub fn mapsequenceengagesummon_commit_hook(this: &MapSequenceEngageSummon, _method_info: OptionalMethod) {
    println!("[mapsequenceengagesummon_commit_hook/BEG]");
    call_original!(this, _method_info);
    println!("[mapsequenceengagesummon_commit_hook/END]");
}

#[unity::from_offset("App", "MapSequenceEngageSummon", "SimpleSummon")]
fn mapsequenceengagesummon_simplesummon(this: &MapSequenceEngageSummon, method_info: OptionalMethod);
#[unity::from_offset("App", "MapSequenceEngageSummon", "MindStart")]
fn mapsequenceengagesummon_mindstart(this: &MapSequenceEngageSummon, method_info: OptionalMethod);
#[unity::from_offset("App", "MapSequenceEngageSummon", "MindEnd")]
fn mapsequenceengagesummon_mindend(this: &MapSequenceEngageSummon, method_info: OptionalMethod);
#[unity::from_offset("App", "MapSequenceEngageSummon", "Commit")]
fn mapsequenceengagesummon_commit(this: &MapSequenceEngageSummon, method_info: OptionalMethod);
#[unity::from_offset("App", "MapSequenceEngageSummon", "CreateBind")]
fn mapsequenceengagesummon_createbind(supper: &ProcInst, method_info: OptionalMethod);
///////////////////////////////////////////////////


// 0x71023cfe10
// void App.MapSequenceEngageSummon$$CreateBind(App_ProcInst_o *super,MethodInfo *method)
#[skyline::hook(offset = 0x023cfe10)]
pub fn mapsequenceengagesummon_createbind_hook(supper: &ProcInst, _method_info: OptionalMethod) {
    println!("[mapsequenceengagesummon_createbind_hook/BEG]");
    call_original!(supper, _method_info);
    println!("[mapsequenceengagesummon_createbind_hook/END]");
}

// 0x71023cfd30
// void App.MapSequenceEngageSummon$$CreateTelop(App_MapSequenceEngageSummon_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x023cfd30)]
pub fn mapsequenceengagesummon_createtelop_hook(this: &MapSequenceEngageSummon, _method_info: OptionalMethod) {
    println!("[mapsequenceengagesummon_createtelop_hook/BEG]");
    call_original!(this, _method_info);
    println!("[mapsequenceengagesummon_createtelop_hook/END]");
}

// 0x71023cf360
// void App.MapSequenceEngageSummon$$set_Person(App_MapSequenceEngageSummon_o *__this,App_PersonData_o *value,MethodInfo *method)
#[skyline::hook(offset = 0x023cf360)]
pub fn mapsequenceengagesummon_setperson_hook(this: &MapSequenceEngageSummon, value: Option<&PersonData>, _method_info: OptionalMethod) {
    println!("[mapsequenceengagesummon_setPerson/BEG]");
    call_original!(this, value, _method_info);
    println!("[mapsequenceengagesummon_setPerson/END]");
}

// 0x71023cf640
// void App.MapSequenceEngageSummon$$Calculate(App_MapSequenceEngageSummon_o *__this,MethodInfo *method)
#[skyline::hook(offset = 0x023cf640)]
pub fn mapsequenceengagesummon_calculate_hook(this: &MapSequenceEngageSummon, _method_info: OptionalMethod) {
    println!("[mapsequenceengagesummon_calculate_hook/BEG]");
    call_original!(this, _method_info);
    println!("[mapsequenceengagesummon_calculate_hook/END]");
}


// 0x7101a0d650
// void App.Unit$$CreateForSummonImpl1(App_Unit_o *__this,App_PersonData_o *person,App_Unit_o *original,int32_t rank, MethodInfo *method)
#[skyline::hook(offset = 0x01a0d650)]
pub fn unit_createforsummonimpl1(this: &Unit, person: Option<&PersonData>, original: Option<&Unit>, rank: i32, _method_info: OptionalMethod) {
    println!("[unit_createforsummonimpl1/BEG]");
    if let Some(person_unwrapped) = person {
        if let Some(original_unwrapped) = original {
            println!("[unit_createforsummonimpl1] person: {} original: {}", Mess::get(person_unwrapped.get_name().unwrap()).to_string(), original_unwrapped.get_pid());
            call_original!(this, person, original, rank, _method_info);
        } else {
            println!("[unit_createforsummonimpl1] there is no original?");
        }
    } else {
        println!("[unit_createforsummonimpl1] no person??? Is this a problem?");
    }

    println!("[unit_createforsummonimpl1/END]");
}
