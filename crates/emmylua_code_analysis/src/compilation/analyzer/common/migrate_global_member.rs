use crate::{DbIndex, GlobalId, LuaDeclId, LuaMemberId, LuaMemberOwner, LuaTypeOwner};

use super::get_owner_id;

pub fn migrate_global_members_when_type_resolve(
    db: &mut DbIndex,
    type_owner: LuaTypeOwner,
) -> Option<()> {
    match type_owner {
        LuaTypeOwner::Decl(decl_id) => {
            migrate_global_member_to_decl(db, decl_id);
        }
        LuaTypeOwner::Member(member_id) => {
            migrate_global_member_to_member(db, member_id);
        }
        _ => {}
    }
    Some(())
}

fn migrate_global_member_to_decl(db: &mut DbIndex, decl_id: LuaDeclId) -> Option<()> {
    // Phase 1: collect everything we need via immutable borrows.
    let (owner_id, name) = {
        let decl = db.get_decl_index().get_decl(&decl_id)?;
        if !decl.is_global() {
            return None;
        }
        let owner_id = get_owner_id(db, &decl_id.into())?;
        let name = decl.get_name().to_string();
        (owner_id, name)
    };

    let global_id = GlobalId::new(&name);

    // Members from GlobalPath("XX") — the primary migration source.
    let global_path_members: Vec<LuaMemberId> = db
        .get_member_index()
        .get_members(&LuaMemberOwner::GlobalPath(global_id))
        .map(|v| v.iter().map(|m| m.get_id()).collect())
        .unwrap_or_default();

    // Cross-decl collection:
    //
    // Case A — owner is a Type (this decl has a `---@type X` annotation):
    //   Also collect members from every other global decl for the same name whose
    //   resolved owner is an Element (i.e. plain `XX = {}`).  This covers the
    //   scenario where a separate file defines the table and a different file
    //   annotates the global with the class type.
    //
    // Case B — owner is an Element (this decl is a plain `XX = {}`):
    //   Find any other global decl for the same name whose resolved owner is a
    //   Type (i.e. `---@type X`).  We will also register the current members
    //   under that Type owner so the class member list stays up to date regardless
    //   of file analysis order.
    let other_global_decl_ids: Vec<LuaDeclId> = db
        .get_global_index()
        .get_global_decl_ids(&name)
        .map(|ids| ids.iter().filter(|&&id| id != decl_id).copied().collect())
        .unwrap_or_default();

    // For Case A: extra members from Element-owned siblings.
    let extra_element_members: Vec<LuaMemberId> = if matches!(owner_id, LuaMemberOwner::Type(_)) {
        let mut extra = Vec::new();
        for &other_id in &other_global_decl_ids {
            if let Some(elem_owner) = get_owner_id(db, &other_id.into()) {
                if matches!(elem_owner, LuaMemberOwner::Element(_)) {
                    if let Some(members) = db.get_member_index().get_members(&elem_owner) {
                        extra.extend(members.iter().map(|m| m.get_id()));
                    }
                }
            }
        }
        extra
    } else {
        Vec::new()
    };

    // For Case B: sibling Type owners to also receive the current members.
    let sibling_type_owners: Vec<LuaMemberOwner> =
        if matches!(owner_id, LuaMemberOwner::Element(_)) {
            other_global_decl_ids
                .iter()
                .filter_map(|&other_id| {
                    let o = get_owner_id(db, &other_id.into())?;
                    matches!(o, LuaMemberOwner::Type(_)).then_some(o)
                })
                .collect()
        } else {
            Vec::new()
        };

    // Phase 2: mutations.
    let member_index = db.get_member_index_mut();

    // Migrate GlobalPath members + extra Element members to owner_id.
    for &member_id in global_path_members.iter().chain(extra_element_members.iter()) {
        member_index.set_member_owner(owner_id.clone(), member_id.file_id, member_id);
        member_index.add_member_to_owner(owner_id.clone(), member_id);
    }

    // For Case B: register GlobalPath members under any sibling Type owner.
    for type_owner in &sibling_type_owners {
        for &member_id in &global_path_members {
            member_index.set_member_owner(type_owner.clone(), member_id.file_id, member_id);
            member_index.add_member_to_owner(type_owner.clone(), member_id);
        }
    }

    Some(())
}

fn migrate_global_member_to_member(db: &mut DbIndex, member_id: LuaMemberId) -> Option<()> {
    let member = db.get_member_index().get_member(&member_id)?;
    let global_id = member.get_global_id()?;
    let owner_id = get_owner_id(db, &member_id.into())?;

    let members = db
        .get_member_index()
        .get_members(&LuaMemberOwner::GlobalPath(global_id.clone()))?
        .iter()
        .map(|member| member.get_id())
        .collect::<Vec<_>>();

    let member_index = db.get_member_index_mut();
    for member_id in members {
        member_index.set_member_owner(owner_id.clone(), member_id.file_id, member_id);
        member_index.add_member_to_owner(owner_id.clone(), member_id);
    }

    Some(())
}
