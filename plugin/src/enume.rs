use unity::{
    prelude::*,
    il2cpp::class::Il2CppRGCTXData
  };
  
  use engage::{
    force::ForceType, gamedata::{ terrain::TerrainData, Gamedata }, map::{
      r#enum::RangeEnumerator,
      image::{MapImage, MapImageCore, MapImageCoreByte}
    }, sequence::mapsequencetargetselect::MapTarget, util::get_instance
  };
  
  // Define our new method as a trait so that we can extend the MapTarget structure without adding the function in the Engage crate
  pub trait DisengageMapTargetEnumerator {
    fn enumerate_disengage(&mut self);
  }
  
  impl DisengageMapTargetEnumerator for MapTarget {
    
    // This function runs through all of the valid spaces around the player
    // unit, within a certain range, then checks any other units found on
    // those spaces.  If they are found to be a valid target, they are added
    // to a list of targets.  If not, it continues looking at the spaces.
    // If there are no valid targets, the steal option does not even appear
    // in the menu.  
    fn enumerate_disengage(&mut self) {
      let mut local_c0 = RangeEnumerator::default();
    
      if self.unit.is_none() {
        println!("self.unit = None");
        return;
      } else {
        println!("self.unit = {}", self.unit.unwrap().person.unit_icon_id.unwrap());
      }
  
      let cur_unit = self.unit.unwrap();
    
      if cur_unit.status.value & 0x10000 != 0 {
        println!("self.unit's Status is funky");
        return;
      }
    
      if (cur_unit.extra_hp_stock_count + cur_unit.hp_stock_count == 0) && (cur_unit.hp_value == 0) {
        println!("self.unit's HP is funky");
        return;
      }
  
      if !cur_unit.has_sid("SID_Steal".into()) {
        return;
      }
    
      let mapimage_instance = get_instance::<MapImage>();
    
      if ((mapimage_instance.playarea_z2 - cur_unit.z as i32) * (cur_unit.z as i32 - mapimage_instance.playarea_z1)) | ((mapimage_instance.playarea_x2 - cur_unit.x as i32) * (cur_unit.x as i32 - mapimage_instance.playarea_x1)) < 0 {
        println!("PlayArea for self.unit is funky");
        return;
      }
    
      let class = get_generic_class!(MapImageCore<u8>).unwrap();
  
      let rgctx = unsafe {
          &*(class.rgctx_data as *const Il2CppRGCTXData as *const u8 as *const [&'static MethodInfo; 5])
      };
  
      let core_get = unsafe {
          std::mem::transmute::<_, extern "C" fn(&MapImageCoreByte, i64) -> u8>(
              rgctx[3].method_ptr,
          )
      };
  
      let result = core_get(mapimage_instance.terrain.m_result, ((cur_unit.x as i32) + ((cur_unit.z as i32) << 5)).into());
    
      let ter_dat = TerrainData::try_index_get(result.into()).unwrap();
  
      if ter_dat.is_not_target() {
        println!("Terrain Data is not a valid target");
        return;
      }
      // Check if the current unit is a player, enemy or ally
      let force_type1 = if cur_unit.force.unwrap().force_type < ForceType::Absent as i32 {
        // Only keep the lowest 5 bits
        cur_unit.force.unwrap().force_type & 0x1f
      } else {
        7
      };
    
      if force_type1 < ForceType::Absent as i32 {
        println!("force is valid");
        let mask_skill = cur_unit.mask_skill.unwrap();
  
        if (cur_unit.status.value & 0x600008000000 == 0) && (mask_skill.flags & 0x14 == 0) && (mask_skill.bad_states & 0x4d0 == 0) {
          let x: i32 = self.x.into();
          let z: i32 = self.z.into();
    
          let x_1 = (x - 1).clamp(mapimage_instance.playarea_x1, mapimage_instance.playarea_x2);
          let z_1 = (z - 1).clamp(mapimage_instance.playarea_z1, mapimage_instance.playarea_z2);
          let x_2 = (x + 1).clamp(mapimage_instance.playarea_x1, mapimage_instance.playarea_x2);
          let z_2 = (z + 1).clamp(mapimage_instance.playarea_z1, mapimage_instance.playarea_z2);
  
          local_c0.max_z = z_2;
          local_c0.current.z = z_2;
          local_c0.current.x = x_1 - 1;
          local_c0.min_x = x_1;
          local_c0.pivot_z = z;
          local_c0.pivot_x = x_1 + 1;
          local_c0.near = 1;
          local_c0.far = 1;
          // lol whatever i'm tired and don't wanna deal with this, pray the gods are benevolent
          // local_c0.m_current.range = x_1 << 0x20;
          (local_c0.current.range, _) = x_1.overflowing_shl(0x20);
          local_c0.max_x = x_2;
          local_c0.min_z = z_1;
  
          // Go through every target in the range
          for target in local_c0.flat_map(|(x, z)| mapimage_instance.get_target_unit(x, z)) {
            println!("Target found: {}", engage::mess::Mess::get(target.person.get_name().unwrap()));
  
            // Check if the forces are similar
            let target_force = target.force.map(|force| force.force_type & 0x1f).unwrap_or(7);
            let unit_force = cur_unit.force.map(|force| force.force_type & 0x1f).unwrap_or(7);
  
            // Unit and target are in the same Force, we are looking for enemies
            if unit_force == target_force {
              continue;
            }
  
            println!("Target is of a different force than unit");
  
            let unit_has_tradables = cur_unit.item_list.unit_items.iter()
              .flatten()
              .any(|unit_item| (unit_item.item.flag.value & 0x80) == 0 && (unit_item.item.flag.value & 0x200) == 0 && unit_item.index | 2 != 2);
  
            let target_has_tradable = target.item_list.unit_items.iter()
              .flatten()
              .any(|unit_item| (unit_item.item.flag.value & 0x80) == 0 && (unit_item.item.flag.value & 0x200) == 0 && unit_item.index | 2 != 2);
  
            if !unit_has_tradables && !target_has_tradable {
              // If both the unit or enemy have nothing valid to steal, this isn't a valid target
              continue;
            }
  
            println!("The unit or target have tradable items");
  
            // Target is not targetable.
            if target.status.value & 0x10000 != 0 {
              println!("Target has the NotTarget status");
              continue;
            }
  
            // Target is somehow dead?
            if (target.extra_hp_stock_count + target.hp_stock_count == 0) && (target.hp_value == 0) {
              println!("Target is dead");
              continue;
            }
  
            // Failsafe: check that the target is within battlefield bounds
            if ((mapimage_instance.playarea_z2 - target.z as i32) * (target.z as i32 - mapimage_instance.playarea_z1)) | ((mapimage_instance.playarea_x2 - target.x as i32) * (target.x as i32 - mapimage_instance.playarea_x1)) < 0 {
              continue;
            }
            println!("Target is within playable bounds");
  
            // Check that player has more speed than target
            if (target.get_capability(3, true) as u8) >= (cur_unit.get_capability(3, true) as u8) {
              continue;
            }
  
  
            // Get the index of the TerrainData where the target is standing
            let terrain_idx = core_get(mapimage_instance.terrain.m_result, ((target.x as i32) + ((target.z as i32) << 5)).into());
    
            if let Some(terrain_data) = TerrainData::try_index_get(terrain_idx.into()) {
              if terrain_data.is_not_target() {
                println!("Terrain Data is not targetable");
                continue;
              }
            }
  
            if ((1 << target_force) & 0x6) == 0 {
              println!("Something wrong with the target's Force");
              continue;
            }
  
            if let Some(mask_skill) = target.mask_skill {
              // Check if the target has Vision, Lockon or Summon flags
              if (target.status.value & 0x600008000000 == 0) && (mask_skill.flags & 0x14 == 0) && (mask_skill.bad_states & 0x4d0 == 0) {
                // Make sure the DataSet is set
                if let Some(dataset) = self.m_dataset.as_mut() {
                  // TODO: We could probably just use the Add method on Stack instead of checking ourselves
                  // Check if the DataSet's Stack has elements in it
                  if dataset.m_stack.len() > 0 {
                    // Add the target to the list of target data
                    if let Some(entry) = dataset.m_stack.pop() {
                      entry.set(target, x, z, 0, -1);
                      dataset.m_list.add(entry);
                    }
                  }
                }
              }
            }
          }
        } else{
          println!("General Return");
          return;
        }
      }
  
      println!("General Return 2");
      //call_original!(this, _method_info);
    }
  }
  