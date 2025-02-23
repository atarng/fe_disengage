#![feature(ptr_sub_ptr)]

use engage::{
    force::ForceType,
    gamedata::{unit::{ Unit, Gender }, skill::SkillData},
    titlebar::TitleBar,
    gamesound::GameSound,
    mapmind::MapMind,
    menu::*,
    proc::{desc::ProcDesc, Bindable, ProcInst},
    sequence::{ mapsequence::human::MapSequenceHuman, mapsequencetargetselect::{MapSequenceTargetSelect, MapTarget} },
    util::{get_instance, get_singleton_proc_instance}
};

use mapunitcommand::{ MapUnitCommandMenu
    , TradeMenuItem
    // , EngageSummonMenuItem
};

use skyline::{ install_hook, patching::Patch, };
use std::sync::OnceLock;

use unity::{ prelude::*, system::List };

mod enume;
use enume::DisengageMapTargetEnumerator;

const DISENGAGE_MIND_TYPE: u32 = 0x38;
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
    __: [u8; 0x38],
    delegates: *const u8,
    // ...
}

impl<T: Bindable> engage::proc::Delegate for ProcVoidMethodMut<T> { }

impl<T: Bindable> ProcVoidMethodMut<T> {
    /// Prepare a ProcVoidMethod using your target and method of choice.
    ///
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

    skyline::install_hooks!(
        mapunitcommandmenu_createbind,
        maptarget_enumerate,
        mapsequencetargetselect_decide_normal,
        mapbattleinforoot_setcommandtext,
        mapsequencehuman_createbind,
    );
}

// Create our new menu command for Disengage
#[unity::hook("App", "MapUnitCommandMenu", "CreateBind")]
pub fn mapunitcommandmenu_createbind(sup: &mut ProcInst, _method_info: OptionalMethod) {
    println!("[mapunitcommandmenu_createbind] BEG");

    let maptarget_instance = get_instance::<MapTarget>();
    let cur_mind = maptarget_instance.m_mind;

    println!("[mapunitcommandmenu_createbind] cur_mind: {}", cur_mind);

    //// 0x7101e518f0
    //// void App.MapUnitCommandMenu.TradeMenuItem$$.ctor(App_MapUnitCommandMenu_TradeMenuItem_o *__this,MethodInfo *method)
    //// Create a new class using TradeMenuItem as reference so that we do not wreck the original command for ours.
    // Create a new class using EngageSummonMenuItem as reference so that we do not wreck the original command for ours.
    let disengage = DISENGAGE_CLASS.get_or_init(|| {
        // EngageSummonMenuItem is a nested class inside of MapUnitCommandMenu, so we need to dig for it.
        let menu_class  = *MapUnitCommandMenu::class()
            .get_nested_types()
            .iter()

            // .find(|class| class.get_name().contains("EngageSummonMenuItem"))
            ////////////////////////////////////
            .find(|class| class.get_name().contains("TradeMenuItem"))

            .unwrap();
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
            .get_virtual_method_mut("get_Mind")
            .map(|method| method.method_ptr = disengage_get_mind as _)
            .unwrap();
        new_class
             .get_virtual_method_mut("get_FlagID")
             .map(|method| method.method_ptr = disengage_get_flagid as _)
             .unwrap();
        new_class
    });

    call_original!(sup, _method_info);
    
    // Instantiate our custom class as if it was EngageSummonMenuItem
    // let instance = Il2CppObject::<EngageSummonMenuItem>::from_class(disengage).unwrap();
    // let menu_item_list = &mut sup.child.as_mut().unwrap().cast_mut::<BasicMenu<EngageSummonMenuItem>>().full_menu_item_list;
    ////////////////////////////////////
    let instance = Il2CppObject::<TradeMenuItem>::from_class(disengage).unwrap();
    let menu_item_list = &mut sup.child.as_mut().unwrap().cast_mut::<BasicMenu<TradeMenuItem>>().full_menu_item_list;

    menu_item_list.insert((menu_item_list.len() - 1) as i32, instance);

    println!("[mapunitcommandmenu_createbind] END");
}

// This is a generic function that essentially checks the Mind value, and then calls
// a more specialized Enumerate function based on the result.
// Enumerate functions are used for checking if there is a valid target in range,
// and making a list of them.
#[unity::hook("App", "MapTarget", "Enumerate")]
pub fn maptarget_enumerate(this: &mut MapTarget, mask: i32, _method_info: OptionalMethod) {
    println!("[maptarget_enumerate] BEG: {}", this.m_mind);
    // match this.m_mind {
    if this.m_mind == DISENGAGE_MIND_TYPE {
        println!("[maptarget_enumerate] disengage");

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

        this.enumerate_disengage();

        if let Some(dataset) = this.m_dataset.as_mut() {
            dataset.m_list
                .iter_mut()
                .enumerate()
                .for_each(|(count_var, data_item)| {
                    data_item.m_index = count_var as i8;    
                });
        }
    } else if this.m_mind == REENGAGE_MIND_TYPE {
        println!("[maptarget_enumerate] reengage");
        // this.m_action_mask = mask as u32;
        // if let Some(unit) = this.unit {
        //     if this.x < 0 {
        //         this.x = unit.x as i8;
        //     }
        //     if this.z < 0 {
        //         this.z = unit.z as i8;
        //     }
        // }
        // if let Some(dataset) = this.m_dataset.as_mut() {
        //     dataset.clear();
        // }
        // this.enumerate_reengage();
        // if let Some(dataset) = this.m_dataset.as_mut() {
        //     dataset.m_list
        //         .iter_mut()
        //         .enumerate()
        //         .for_each(|(count_var, data_item)| {
        //             data_item.m_index = count_var as i8;    
        //         });
        // }
    } else {
        println!("[maptarget_enumerate] mind: {}", this.m_mind);
        call_original!(this, mask, _method_info);
    }
    println!("[maptarget_enumerate] END");
}


// This function is... interesting.  It essentially builds a BIG list of labels and functions to run.
// The labels are a way for the game to jump around the list and then run a series of functions in a row.
// This is essentially how the ENTIRE game functions to some degree.
// What we're doing here is adding a new section of entries to the list specifically for the Steal command.
// We insert the new function calls and labels in reverse order because adding something to an existing index
// pushes whatever was already there forward, and also makes later additions simpler.
#[skyline::hook(offset = 0x2677780)]
pub fn mapsequencehuman_createbind(sup: &mut MapSequence, is_resume: bool, _method_info: OptionalMethod) {
    println!("[mapsequencehuman_createbind] BEG");

    call_original!(sup, is_resume, _method_info);

    let mut vec = unsafe { (*(sup.child)).descs.to_vec() };

    let desc = engage::proc::desc::ProcDesc::jump(0x10);
    vec.insert(0x9a, desc);

    let method = mapsequencehuman_postitemmenutrade::get_ref();
    let method = unsafe { std::mem::transmute(method.method_ptr) };
    let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    vec.insert(0x9a, desc);

    let method = mapitemmenu_createbindtrade::get_ref();
    let method = unsafe { std::mem::transmute(method.method_ptr) };
    let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    vec.insert(0x9a, desc);

    let method = mapsequencehuman_preitemmenutrade::get_ref();
    let method = unsafe { std::mem::transmute(method.method_ptr) };
    let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    vec.insert(0x9a, desc);

    let method = mapsequencehuman_unitmenuprepare::get_ref();
    let method = unsafe { std::mem::transmute(method.method_ptr) };
    let desc = unsafe { ProcDesc::call(ProcVoidMethodMut::new(&mut (*sup.child), method)) };
    vec.insert(0x9a, desc);

    // WHERE DOES 53 come from???
    let disengage_label = ProcDesc::label(53);
    vec.insert(0x9a, disengage_label);

    let new_descs = Il2CppArray::from_slice(vec).unwrap();
    unsafe { (*sup.child).descs = new_descs };

    println!("[mapsequencehuman_createbind] END");
}

// Make "Steal" appear on the preview when highlighting an enemy to steal from.
// This function is what sets the text that appears in between the two windows
// when highlighting an enemy.
#[unity::hook("App", "MapBattleInfoRoot", "SetCommandText")]
pub fn mapbattleinforoot_setcommandtext(this: &mut MapBattleInfoRoot, mind_type: i32, _method_info: OptionalMethod) {
    println!("[mapbattleinforoot_setcommandtext/BEG]");
    if DISENGAGE_MIND_TYPE == mind_type.try_into().unwrap() {
        InfoUtil::try_set_text(&this.command_text, "Disengage");
    } else if REENGAGE_MIND_TYPE == mind_type.try_into().unwrap() {
        InfoUtil::try_set_text(&this.command_text, "Reengage");
    } else {
        call_original!(this, mind_type, _method_info);
    }
    println!("[mapbattleinforoot_setcommandtext/END]");
}

// This is the function that usually runs when you press A while highlighting a target and the
// forecast windows are up.
#[unity::hook("App", "MapSequenceTargetSelect", "DecideNormal")]
pub fn mapsequencetargetselect_decide_normal(this: &mut MapSequenceTargetSelect, _method_info: OptionalMethod) {
    println!("[mapsequencetargetselect_decide_normal] BEG");

    let maptarget_instance = get_instance::<MapTarget>();
    let cur_mind = maptarget_instance.m_mind;
    if cur_mind == 0x38 {
        println!("[mapsequencetargetselect_decide_normal] STEAL/DISENGAGE ({})", cur_mind);

        let mapmind_instance = get_instance::<MapMind>();
        let mut unit_index = 7;
        if this.can_select_target() && this.target_data.is_some() {
            unit_index = this.target_data.unwrap().m_unit.index;
        }

        mapmind_instance.set_trade_unit_index(unit_index as _);
        
        let mapsequencehuman_instance = get_singleton_proc_instance::<MapSequenceHuman>().unwrap();
        // This is using the new label added in the mapsequencehuman_createbind.
        // 0x35: 53
        ProcInst::jump(mapsequencehuman_instance, 0x35);

        GameSound::post_event("Decide", None);
    } else {
        println!("[mapsequencetargetselect_decide_normal] mind: {}", cur_mind);

        call_original!(this, _method_info)
    }

    println!("[mapsequencetargetselect_decide_normal] END");
}

pub extern "C" fn disengage_get_name(_this: &(), _method_info: OptionalMethod) -> &'static Il2CppString { "Disengage".into() }

pub extern "C" fn disengage_get_desc(_this: &(), _method_info: OptionalMethod) -> &'static Il2CppString {
    "Summon equipped emblem to fight with you.".into()
}

// what does 0x38 mean??? Decimal: 56 binary: 0b111000
// dump.cs: public enum MapMind.Type // TypeDefIndex: 12219
pub extern "C" fn disengage_get_mind(_this: &(), _method_info: OptionalMethod) -> i32 {
    DISENGAGE_MIND_TYPE.try_into().unwrap()
}

pub extern "C" fn disengage_get_flagid(_this: &(), _method_info: OptionalMethod) -> &'static Il2CppString {
    "EmblemSummon".into()
}

// Functions for the ProcDesc fuckening
#[unity::from_offset("App", "MapSequenceHuman", "UnitMenuPrepare")]
fn mapsequencehuman_unitmenuprepare(this: &MapSequenceHuman, method_info: OptionalMethod);

#[unity::from_offset("App", "MapSequenceHuman", "PreItemMenuTrade")]
fn mapsequencehuman_preitemmenutrade(this: &MapSequenceHuman, method_info: OptionalMethod);

#[unity::from_offset("App", "MapItemMenu", "CreateBindTrade")]
fn mapitemmenu_createbindtrade(sup: &ProcInst, method_info: OptionalMethod);

#[unity::from_offset("App", "MapSequenceHuman", "PostItemMenuTrade")]
fn mapsequencehuman_postitemmenutrade(this: &MapSequenceHuman, method_info: OptionalMethod);

#[unity::from_offset("App", "MapSituation", "GetPlayer")]
fn mapsituation_get_player(this: &MapSituation, forcetype: i32, method_info: OptionalMethod) -> i32;
