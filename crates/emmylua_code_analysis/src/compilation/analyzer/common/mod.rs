mod migrate_global_member;
use migrate_global_member::migrate_global_members_when_type_resolve;
use rowan::TextRange;

use emmylua_parser::{LuaAstNode, LuaExpr};

use crate::{
    FileId, InFiled, LuaMemberId, LuaTypeCache, LuaTypeOwner,
    db_index::{DbIndex, LuaMemberOwner, LuaType, LuaTypeDeclId},
};

pub fn bind_type(
    db: &mut DbIndex,
    type_owner: LuaTypeOwner,
    mut type_cache: LuaTypeCache,
) -> Option<()> {
    let decl_type_cache = db.get_type_index().get_type_cache(&type_owner);

    if decl_type_cache.is_none() {
        // type backward
        if type_cache.is_infer()
            && let LuaTypeOwner::Decl(decl_id) = &type_owner
            && let Some(decl_ref) = db
                .get_reference_index()
                .get_decl_references(&decl_id.file_id, decl_id)
            && decl_ref.mutable
        {
            match &type_cache.as_type() {
                LuaType::IntegerConst(_) => type_cache = LuaTypeCache::InferType(LuaType::Integer),
                LuaType::StringConst(_) => type_cache = LuaTypeCache::InferType(LuaType::String),
                LuaType::BooleanConst(_) => type_cache = LuaTypeCache::InferType(LuaType::Boolean),
                LuaType::FloatConst(_) => type_cache = LuaTypeCache::InferType(LuaType::Number),
                _ => {}
            }
        }

        db.get_type_index_mut()
            .bind_type(type_owner.clone(), type_cache);
        migrate_global_members_when_type_resolve(db, type_owner);
    } else {
        let decl_type = decl_type_cache?.as_type();
        merge_def_type(db, decl_type.clone(), type_cache.as_type().clone(), 0);
    }

    Some(())
}

fn merge_def_type(db: &mut DbIndex, decl_type: LuaType, expr_type: LuaType, merge_level: i32) {
    if merge_level > 1 {
        return;
    }

    if let LuaType::Def(def) = &decl_type {
        match &expr_type {
            LuaType::TableConst(in_filed_range) => {
                merge_def_type_with_table(db, def.clone(), in_filed_range.clone());
            }
            LuaType::Instance(instance) => {
                let base_ref = instance.get_base();
                merge_def_type(db, base_ref.clone(), expr_type, merge_level + 1);
            }
            _ => {}
        }
    }
}

fn merge_def_type_with_table(
    db: &mut DbIndex,
    def_id: LuaTypeDeclId,
    table_range: InFiled<TextRange>,
) -> Option<()> {
    let expr_member_owner = LuaMemberOwner::Element(table_range);
    let member_index = db.get_member_index_mut();
    let expr_member_ids = member_index
        .get_members(&expr_member_owner)?
        .iter()
        .map(|member| member.get_id())
        .collect::<Vec<_>>();
    let def_owner = LuaMemberOwner::Type(def_id);
    for table_member_id in expr_member_ids {
        add_member(db, def_owner.clone(), table_member_id);
    }

    Some(())
}

pub fn add_member(db: &mut DbIndex, owner: LuaMemberOwner, member_id: LuaMemberId) -> Option<()> {
    db.get_member_index_mut()
        .set_member_owner(owner.clone(), member_id.file_id, member_id);
    db.get_member_index_mut()
        .add_member_to_owner(owner.clone(), member_id);

    Some(())
}

fn get_owner_id(db: &DbIndex, type_owner: &LuaTypeOwner) -> Option<LuaMemberOwner> {
    let type_cache = db.get_type_index().get_type_cache(type_owner)?;
    match type_cache.as_type() {
        LuaType::Ref(type_id) => Some(LuaMemberOwner::Type(type_id.clone())),
        LuaType::TableConst(id) => Some(LuaMemberOwner::Element(id.clone())),
        LuaType::Instance(inst) => Some(LuaMemberOwner::Element(inst.get_range().clone())),
        _ => None,
    }
}

/// Returns `true` when `prefix_expr` is a global variable whose type was
/// explicitly annotated with a doc-comment (`---@type X`, etc.), **and**
/// the annotation is declared in the same file (`file_id`) as the call-site.
///
/// Only same-file assignments are treated as class-extension definitions
/// (e.g. `---@type XX; XX = {}; XX.A1 = 1` in a single file).  Assignments
/// from other files are treated as usage and should not extend the class.
pub fn prefix_is_doc_annotated_global(
    db: &DbIndex,
    file_id: FileId,
    prefix_expr: &LuaExpr,
) -> bool {
    let LuaExpr::NameExpr(name_expr) = prefix_expr else {
        return false;
    };
    let name = match name_expr.get_name_text() {
        Some(n) => n,
        None => return false,
    };
    // A locally shadowed name does not refer to the global.
    let is_shadowed = db
        .get_decl_index()
        .get_decl_tree(&file_id)
        .and_then(|tree| tree.find_local_decl(&name, name_expr.get_position()))
        .map(|decl| decl.is_local() || decl.is_implicit_self())
        .unwrap_or(false);
    if is_shadowed {
        return false;
    }
    // Check whether a global decl for this name in the CURRENT file has a
    // doc-type annotation.  Cross-file assignments are considered usage, not
    // class definition.
    if let Some(decl_ids) = db.get_global_index().get_global_decl_ids(&name) {
        for decl_id in decl_ids {
            if decl_id.file_id != file_id {
                continue;
            }
            if let Some(type_cache) = db.get_type_index().get_type_cache(&(*decl_id).into()) {
                return type_cache.is_doc();
            }
        }
    }
    false
}
