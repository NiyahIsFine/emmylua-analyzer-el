#[cfg(test)]
mod test {
    use smol_str::SmolStr;

    use crate::{LuaType, LuaUnionType, VirtualWorkspace};

    #[test]
    fn test_issue_318() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        ws.def(
            r#"
        local map = {
            a = 'a',
            b = 'b',
            c = 'c',
        }
        local key      --- @type string
        c = map[key]   -- type should be ('a'|'b'|'c'|nil)

        "#,
        );

        let c_ty = ws.expr_ty("c");

        let union_type = LuaType::Union(
            LuaUnionType::from_vec(vec![
                LuaType::StringConst(SmolStr::new("a").into()),
                LuaType::StringConst(SmolStr::new("b").into()),
                LuaType::StringConst(SmolStr::new("c").into()),
                LuaType::Nil,
            ])
            .into(),
        );

        assert_eq!(c_ty, union_type);
    }

    #[test]
    fn test_issue_314_generic_inheritance() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        ws.def(
            r#"
        ---@class foo<T>: T
        local foo_mt = {}

        ---@type foo<{a: string}>
        local bar = { a = 'test' }

        c = bar.a -- should be string

        ---@class buz<T>: foo<T>
        local buz_mt = {}

        ---@type buz<{a: integer}>
        local qux = { a = 5 }

        d = qux.a -- should be integer
        "#,
        );

        let c_ty = ws.expr_ty("c");
        let d_ty = ws.expr_ty("d");

        assert_eq!(c_ty, LuaType::String);
        assert_eq!(d_ty, LuaType::Integer);
    }

    #[test]
    fn test_issue_397() {
        let mut ws = VirtualWorkspace::new();

        ws.def(
            r#"
        --- @class A
        --- @field field? integer

        --- @class B : A
        --- @field field integer

        --- @type B
        local b = { field = 1 }

        local key1 --- @type 'field'
        local key2 = 'field'

        a = b.field -- type is integer - correct
        d = b['field'] -- type is integer - correct
        e = b[key1] -- type is integer? - wrong
        f = b[key2] -- type is integer? - wrong
        "#,
        );

        let a_ty = ws.expr_ty("a");
        let d_ty = ws.expr_ty("d");
        let e_ty = ws.expr_ty("e");
        let f_ty = ws.expr_ty("f");

        assert_eq!(a_ty, LuaType::Integer);
        assert_eq!(d_ty, LuaType::Integer);
        assert_eq!(e_ty, LuaType::Integer);
        assert_eq!(f_ty, LuaType::Integer);
    }

    #[test]
    fn test_keyof() {
        let mut ws = VirtualWorkspace::new();

        ws.def(
            r#"
        ---@class SuiteHooks
        ---@field beforeAll string
        ---@field afterAll number

        ---@type SuiteHooks
        local hooks = {}

        ---@type keyof SuiteHooks
        local name = "beforeAll"

        A = hooks[name]
        "#,
        );

        let ty = ws.expr_ty("A");
        let expected =
            LuaType::Union(LuaUnionType::from_vec(vec![LuaType::String, LuaType::Number]).into());
        assert_eq!(ty, expected);
    }

    #[test]
    fn test_local_shadow_global_member_owner() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        ws.def(
            r#"
        local table = {}
        table.unpack = 1
        A = table.unpack
        "#,
        );

        assert_eq!(ws.expr_ty("A"), LuaType::IntegerConst(1));
    }

    #[test]
    fn test_assign_table_literal_preserves_class_fields() {
        let mut ws = VirtualWorkspace::new();

        ws.def(
            r#"
        ---@class A
        ---@field a string
        ---@field b? number

        ---@type A
        local a
        a = { a = "hello" }

        c = a.a
        "#,
        );

        assert_eq!(
            ws.expr_ty("c"),
            LuaType::StringConst(SmolStr::new("hello").into())
        );
    }

    #[test]
    fn test_assign_object_return_preserves_class_fields() {
        let mut ws = VirtualWorkspace::new();

        ws.def(
            r#"
        ---@class A
        ---@field a string|number
        ---@field b number

        ---@return {a: string}
        local function make()
            return { a = "hello" }
        end

        ---@type A
        local a
        a = make()

        c = a.a
        d = a.b
        "#,
        );

        assert_eq!(ws.expr_ty("c"), LuaType::String);
        assert_eq!(ws.expr_ty("d"), LuaType::Number);
    }

    #[test]
    fn test_assign_table_literal_preserves_class_fields_from_antecedent() {
        let mut ws = VirtualWorkspace::new();

        ws.def(
            r#"
        ---@class A
        ---@field a string
        ---@field b? number

        ---@type A
        local global_a

        ---@return A
        local function make()
            return global_a
        end

        local a = make()
        a = { a = "hello" }

        c = a.a
        "#,
        );

        assert_eq!(
            ws.expr_ty("c"),
            LuaType::StringConst(SmolStr::new("hello").into())
        );
    }

    #[test]
    fn test_assign_from_nil_uses_expr_type() {
        let mut ws = VirtualWorkspace::new();

        ws.def(
            r#"
        local a
        a = "hello"
        b = a
        "#,
        );

        assert_eq!(
            ws.expr_ty("b"),
            LuaType::StringConst(SmolStr::new("hello").into())
        );
    }

    #[test]
    fn test_global_member_owner_prefers_declared_type() {
        let mut ws = VirtualWorkspace::new();
        ws.def(
            r#"
        ---@class Foo
        ---@field existing string

        ---@type Foo
        Foo = {}
        Foo.extra = 1

        ---@type Foo
        local other

        A = other.extra
        "#,
        );

        // Foo is a global annotated with ---@type Foo.
        // Field assignments like `Foo.extra = 1` extend class Foo (FileDefine feature),
        // so `other.extra` (another instance of Foo) should be inferred.
        let a_ty = ws.expr_ty("A");
        assert_ne!(
            a_ty,
            LuaType::Nil,
            "other.extra should be inferred after Foo.extra = 1 extends class Foo, got: {:?}",
            a_ty
        );
    }

    #[test]
    fn test_non_name_prefix_uses_inferred_type() {
        let mut ws = VirtualWorkspace::new();
        ws.def(
            r#"
        local t = {}
        (t).bar = "hi"
        A = t.bar
        "#,
        );

        assert_eq!(
            ws.expr_ty("A"),
            LuaType::StringConst(SmolStr::new("hi").into())
        );
    }

    #[test]
    fn test_table_expr_key_string() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        ws.def(
            r#"
        local key = tostring(1)
        local t = { [key] = 1 }
        value = t[key]
        "#,
        );

        let value_ty = ws.expr_ty("value");
        assert!(
            matches!(value_ty, LuaType::Integer | LuaType::IntegerConst(_)),
            "expected integer type, got {:?}",
            value_ty
        );
    }

    #[test]
    fn test_table_expr_key_doc_const() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        ws.def(
            r#"
        ---@type 'field'
        local key = "field"
        local t = { [key] = 1 }
        value = t[key]
        "#,
        );

        let value_ty = ws.expr_ty("value");
        assert!(
            matches!(value_ty, LuaType::Integer | LuaType::IntegerConst(_)),
            "expected integer type, got {:?}",
            value_ty
        );
    }

    #[test]
    fn test_union_member_access_preserves_never() {
        let mut ws = VirtualWorkspace::new();

        ws.def(
            r#"
        ---@class A
        ---@field y never

        ---@class B
        ---@field y never

        ---@return A|B
        local function make() end

        local value = make()

        result = value.y
        "#,
        );

        assert_eq!(ws.expr_ty("result"), ws.ty("never"));
    }

    #[test]
    fn test_table_expr_index_preserves_never() {
        let mut ws = VirtualWorkspace::new();

        ws.def(
            r#"
        ---@return { y: number } & { y: string }
        local function impossible() end

        local t = {
            a = impossible().y,
        }

        result = t["a"]
        "#,
        );

        assert_eq!(ws.expr_ty("result"), ws.ty("never"));
    }

    /// Test that methods defined on a `---@type ClassName` local variable are
    /// collected as members of `ClassName` (partial class contribution via @type).
    #[test]
    fn test_type_annotation_method_extends_class() {
        let mut ws = VirtualWorkspace::new();
        ws.def(
            r#"
        ---@class HomePlantBLL
        local HomePlantBLL = {}

        ---@type HomePlantBLL
        local HomePlant_Inventory = {}

        function HomePlant_Inventory:OnInit_Inventory()
        end

        ---@type HomePlantBLL
        local inst = HomePlantBLL

        R = inst.OnInit_Inventory
        "#,
        );

        // The method defined on the @type-annotated variable should be
        // visible on HomePlantBLL instances.
        assert_ne!(ws.expr_ty("R"), LuaType::Nil);
    }

    /// Test that field assignments on a `---@type ClassName` variable do NOT
    /// extend the class (only method definitions should).
    #[test]
    fn test_type_annotation_field_does_not_extend_class() {
        let mut ws = VirtualWorkspace::new();
        ws.def(
            r#"
        ---@class MyClass2
        local MyClass2 = {}

        ---@type MyClass2
        local obj = {}
        obj.someField = 42

        ---@type MyClass2
        local inst2 = MyClass2

        R2 = inst2.someField
        "#,
        );

        // Field assignments on @type-annotated variables should NOT extend the class.
        assert_eq!(ws.expr_ty("R2"), LuaType::Nil);
    }

    /// Test that multi-file partial class pattern works with @type annotation.
    #[test]
    fn test_type_annotation_partial_class_multifile() {
        let mut ws = VirtualWorkspace::new();
        ws.def_file(
            "main.lua",
            r#"
        ---@class BLL
        local BLL = {}
        "#,
        );
        ws.def_file(
            "partial.lua",
            r#"
        ---@type BLL
        local Partial = {}

        function Partial:OnInit()
        end
        "#,
        );
        ws.def_file(
            "usage.lua",
            r#"
        ---@type BLL
        local bll = {}

        R3 = bll.OnInit
        "#,
        );

        assert_ne!(ws.expr_ty("R3"), LuaType::Nil);
    }

    /// Test that fields assigned via `self.field = value` inside a method on a
    /// `---@type ClassName` variable are collected as members of `ClassName`.
    #[test]
    fn test_type_annotation_self_field_extends_class() {
        let mut ws = VirtualWorkspace::new();
        ws.def(
            r#"
        ---@class XXX
        local XXX_1 = {}

        function XXX_1:Fun1()
            self.x = 2
        end

        ---@type XXX
        local XXX_2 = {}

        function XXX_2:Fun2()
            self.x2 = 2
        end

        ---@type XXX
        local inst = XXX_1

        R_x = inst.x
        R_x2 = inst.x2
        "#,
        );

        // Both x (from @class method) and x2 (from @type method) should be visible
        assert_ne!(ws.expr_ty("R_x"), LuaType::Nil);
        assert_ne!(ws.expr_ty("R_x2"), LuaType::Nil);
    }

    #[test]
    fn test_cross_file_global_table_member() {
        let mut ws = VirtualWorkspace::new();

        ws.def(
            r#"
        G1 = {}
        G1.a = 1
        "#,
        );

        ws.def(
            r#"
        B = G1.a
        "#,
        );

        let b_ty = ws.expr_ty("B");
        assert_ne!(
            b_ty,
            LuaType::Unknown,
            "Cross-file global table member should be inferred, got: {:?}",
            b_ty
        );
        assert_ne!(
            b_ty,
            LuaType::Nil,
            "Cross-file global table member should not be nil, got: {:?}",
            b_ty
        );
    }

    #[test]
    fn test_cross_file_global_table_member_batch_load() {
        let mut ws = VirtualWorkspace::new();

        ws.def_files(vec![
            (
                "file_a.lua",
                r#"
        G2 = {}
        G2.b = 2
        "#,
            ),
            (
                "file_b.lua",
                r#"
        C = G2.b
        "#,
            ),
        ]);

        let c_ty = ws.expr_ty("C");
        assert_ne!(
            c_ty,
            LuaType::Unknown,
            "Batch-loaded cross-file global table member should be inferred, got: {:?}",
            c_ty
        );
        assert_ne!(
            c_ty,
            LuaType::Nil,
            "Batch-loaded cross-file global table member should not be nil, got: {:?}",
            c_ty
        );
    }

    // Test with reversed file order (file_b defines the usage, file_a defines the global).
    // In a sorted batch load, file_b (lower FileId) may be processed before file_a.
    #[test]
    fn test_cross_file_global_table_member_reverse_order() {
        let mut ws = VirtualWorkspace::new();

        // file_b.lua is given first, so it gets a lower FileId and is analyzed first
        ws.def_files(vec![
            (
                "file_b.lua",
                r#"
        D = G3.c
        "#,
            ),
            (
                "file_a.lua",
                r#"
        G3 = {}
        G3.c = 3
        "#,
            ),
        ]);

        let d_ty = ws.expr_ty("D");
        assert_ne!(
            d_ty,
            LuaType::Unknown,
            "Reverse-order cross-file global table member should be inferred, got: {:?}",
            d_ty
        );
        assert_ne!(
            d_ty,
            LuaType::Nil,
            "Reverse-order cross-file global table member should not be nil, got: {:?}",
            d_ty
        );
    }

    // Test that accessing global table members from file_b works even when
    // table definition and member assignments are in file_a (single-file update path).
    #[test]
    fn test_cross_file_global_member_via_single_file_update() {
        let mut ws = VirtualWorkspace::new();
        ws.def_file(
            "file_a.lua",
            r#"
TABLE1 = {}
TABLE1.M1 = 1
"#,
        );

        let m1_ty = ws.expr_ty("TABLE1.M1");
        assert_ne!(
            m1_ty,
            LuaType::Unknown,
            "TABLE1.M1 should be inferred after def_file, got: {:?}",
            m1_ty
        );
        assert_ne!(
            m1_ty,
            LuaType::Nil,
            "TABLE1.M1 should not be nil, got: {:?}",
            m1_ty
        );
    }

    // Test that table defined in one file and members defined in another file are
    // accessible from a third file (split-definition pattern).
    #[test]
    fn test_cross_file_split_definition() {
        let mut ws = VirtualWorkspace::new();
        ws.def_files(vec![
            ("file_a.lua", "G4 = {}"),
            ("file_b.lua", "G4.x = 10"),
        ]);

        let x_ty = ws.expr_ty("G4.x");
        assert_ne!(
            x_ty,
            LuaType::Unknown,
            "G4.x should be inferred when G4 and G4.x are in different files, got: {:?}",
            x_ty
        );
    }

    // Test that non-method field assignments on a `---@type X`-annotated **global** variable
    // extend class X so fields are visible in completion and type inference.
    // e.g. `---@type XX; XX = {}; XX.A1 = 1` should add `A1` to class `XX`.
    #[test]
    fn test_at_type_annotated_global_field_extends_class() {
        let mut ws = VirtualWorkspace::new();
        ws.def(
            r#"
---@class XX

---@type XX
XX = {}
XX.A1 = 1
XX.A2 = 2
"#,
        );

        let a1_ty = ws.expr_ty("XX.A1");
        assert_ne!(
            a1_ty,
            LuaType::Unknown,
            "XX.A1 should be inferred after @type-annotated global assignment extends class XX, got: {:?}",
            a1_ty
        );
        assert_ne!(a1_ty, LuaType::Nil, "XX.A1 should not be nil, got: {:?}", a1_ty);
    }

    // Cross-file variant matching the user-reported scenario:
    //   File B: defines class XX
    //   File C: `---@type XX; XX = {}; XX.A1 = 1; XX.A2 = 2`
    //   File A: `XX.A1` / `XX.A2` should be inferred
    #[test]
    fn test_at_type_annotated_global_cross_file_extends_class() {
        let mut ws = VirtualWorkspace::new();
        ws.def_files(vec![
            (
                "file_b.lua",
                r#"
---@class XX
XX = "XX"
"#,
            ),
            (
                "file_c.lua",
                r#"
---@type XX
XX = {}
XX.A1 = 1
XX.A2 = 2
"#,
            ),
        ]);

        let a1_ty = ws.expr_ty("XX.A1");
        assert_ne!(
            a1_ty,
            LuaType::Unknown,
            "XX.A1 should be inferred from cross-file @type-annotated global, got: {:?}",
            a1_ty
        );
        assert_ne!(a1_ty, LuaType::Nil, "XX.A1 should not be nil, got: {:?}", a1_ty);

        let a2_ty = ws.expr_ty("XX.A2");
        assert_ne!(
            a2_ty,
            LuaType::Unknown,
            "XX.A2 should be inferred from cross-file @type-annotated global, got: {:?}",
            a2_ty
        );
    }

    // User-reported scenario (problem statement):
    // File A: plain table `XX = {}; XX.A = 1; XX.B = 2; XX.C = 3`  (no annotation)
    // File B: `---@class XX Desc` defined somewhere
    // File C: `---@type XX Des; XX = require("A")`
    // File D: `XX.` should complete A, B, C
    //
    // This tests both file analysis orderings (file_a before file_c and vice versa).
    #[test]
    fn test_at_type_annotated_global_require_cross_file() {
        // Order 1: plain table file analyzed before the @type file.
        let mut ws = VirtualWorkspace::new();
        ws.def_files(vec![
            (
                "file_b.lua",
                r#"
---@class XX
"#,
            ),
            (
                "file_a.lua",
                r#"
XX = {}
XX.A = 1
XX.B = 2
XX.C = 3
"#,
            ),
            (
                "file_c.lua",
                r#"
---@type XX
XX = require("file_a")
"#,
            ),
        ]);

        let a_ty = ws.expr_ty("XX.A");
        assert_ne!(
            a_ty,
            LuaType::Unknown,
            "XX.A should be inferred when plain table is in file_a and @type XX is in file_c (order 1), got: {:?}",
            a_ty
        );
        assert_ne!(a_ty, LuaType::Nil, "XX.A should not be nil (order 1), got: {:?}", a_ty);

        let c_ty = ws.expr_ty("XX.C");
        assert_ne!(
            c_ty,
            LuaType::Unknown,
            "XX.C should be inferred (order 1), got: {:?}",
            c_ty
        );
    }

    #[test]
    fn test_at_type_annotated_global_require_cross_file_reversed() {
        // Order 2: @type file analyzed before the plain table file.
        let mut ws = VirtualWorkspace::new();
        ws.def_files(vec![
            (
                "file_b.lua",
                r#"
---@class XX
"#,
            ),
            (
                "file_c.lua",
                r#"
---@type XX
XX = require("file_a")
"#,
            ),
            (
                "file_a.lua",
                r#"
XX = {}
XX.A = 1
XX.B = 2
XX.C = 3
"#,
            ),
        ]);

        let a_ty = ws.expr_ty("XX.A");
        assert_ne!(
            a_ty,
            LuaType::Unknown,
            "XX.A should be inferred when plain table file_a is analyzed after @type file_c, got: {:?}",
            a_ty
        );
        assert_ne!(
            a_ty,
            LuaType::Nil,
            "XX.A should not be nil (order 2), got: {:?}",
            a_ty
        );
    }
}
