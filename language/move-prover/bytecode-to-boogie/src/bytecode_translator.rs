// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! This module translates the bytecode of a module to Boogie code.

use std::collections::BTreeSet;

use itertools::Itertools;
use log::info;

use libra_types::account_address::AccountAddress;
use libra_types::language_storage::ModuleId;
use stackless_bytecode_generator::{
    stackless_bytecode::StacklessBytecode::{self, *},
    stackless_bytecode_generator::{StacklessFunction, StacklessModuleGenerator},
};

use crate::boogie_helpers::{
    boogie_field_name, boogie_function_name, boogie_local_type, boogie_struct_name,
    boogie_struct_type_value, boogie_type_check, boogie_type_value, boogie_type_values,
};
use crate::code_writer::CodeWriter;
use crate::env::{
    FunctionEnv, GlobalEnv, GlobalType, ModuleEnv, Parameter, StructEnv, TypeParameter,
};
use crate::spec_translator::SpecTranslator;
use ir_to_bytecode_syntax::ast::Loc;

pub struct BoogieTranslator<'env> {
    env: &'env GlobalEnv,
    writer: &'env CodeWriter,
}

pub struct ModuleTranslator<'env> {
    writer: &'env CodeWriter,
    module_env: ModuleEnv<'env>,
    stackless_bytecode: Vec<StacklessFunction>,
}

/// Returns true if for the module no code should be produced because its already defined
/// in the prelude.
pub fn is_module_provided_by_prelude(id: &ModuleId) -> bool {
    id.name().as_str() == "Vector"
        && *id.address() == AccountAddress::from_hex_literal("0x0").unwrap()
}

impl<'env> BoogieTranslator<'env> {
    pub fn new(env: &'env GlobalEnv, writer: &'env CodeWriter) -> Self {
        Self { env, writer }
    }

    pub fn translate(&mut self) {
        // generate definitions for all modules.
        for module_env in self.env.get_modules() {
            ModuleTranslator::new(self, module_env).translate();
        }
    }
}

impl<'env> ModuleTranslator<'env> {
    /// Creates a new module translator. Calls the stackless bytecode generator and wraps
    /// result into the translator.
    fn new(parent: &'env BoogieTranslator, module: ModuleEnv<'env>) -> Self {
        let stackless_bytecode =
            StacklessModuleGenerator::new(module.get_verified_module().as_inner())
                .generate_module();
        Self {
            writer: parent.writer,
            module_env: module,
            stackless_bytecode,
        }
    }

    /// Translates this module.
    fn translate(&mut self) {
        if !is_module_provided_by_prelude(self.module_env.get_id()) {
            info!("translating module {}", self.module_env.get_id().name());
            self.writer
                .set_location(self.module_env.get_module_idx(), Loc::default());
            self.translate_structs();
            self.translate_functions();
        }
    }

    /// Translates all structs in the module.
    fn translate_structs(&self) {
        emitln!(
            self.writer,
            "\n\n// ** structs of module {}\n",
            self.module_env.get_id().name()
        );
        for struct_env in self.module_env.get_structs() {
            self.writer
                .set_location(self.module_env.get_module_idx(), struct_env.get_loc());
            self.translate_struct_type(&struct_env);
            if !struct_env.is_native() {
                self.translate_struct_accessors(&struct_env);
            }
        }
    }

    /// Translates the given struct.
    fn translate_struct_type(&self, struct_env: &StructEnv<'_>) {
        // Emit TypeName
        let struct_name = boogie_struct_name(&struct_env);
        emitln!(self.writer, "const unique {}: TypeName;", struct_name);

        // Emit FieldNames
        for (i, field_env) in struct_env.get_fields().enumerate() {
            let field_name = boogie_field_name(&field_env);
            emitln!(
                self.writer,
                "const {}: FieldName;\naxiom {} == {};",
                field_name,
                field_name,
                i
            );
        }

        // Emit TypeValue constructor function.
        let type_args = struct_env
            .get_type_parameters()
            .iter()
            .enumerate()
            .map(|(i, _)| format!("tv{}: TypeValue", i))
            .join(", ");
        let mut field_types = String::from("EmptyTypeValueArray");
        for field_env in struct_env.get_fields() {
            field_types = format!(
                "ExtendTypeValueArray({}, {})",
                field_types,
                boogie_type_value(self.module_env.env, &field_env.get_type())
            );
        }
        let type_value = format!("StructType({}, {})", struct_name, field_types);
        if struct_name == "LibraAccount_T" {
            // Special treatment of well-known resource LibraAccount_T. The type_value
            // function is forward-declared in the prelude, here we only add an axiom for
            // it.
            emitln!(
                self.writer,
                "axiom {}_type_value() == {};",
                struct_name,
                type_value
            );
        } else {
            emitln!(
                self.writer,
                "function {}_type_value({}): TypeValue {{\n    {}\n}}",
                struct_name,
                type_args,
                type_value
            );
        }
    }

    /// Translates struct accessors (pack/unpack).
    fn translate_struct_accessors(&self, struct_env: &StructEnv<'_>) {
        // Pack function
        let type_args_str = struct_env
            .get_type_parameters()
            .iter()
            .map(|TypeParameter(ref i, _)| format!("{}: TypeValue", i))
            .join(", ");
        let args_str = struct_env
            .get_fields()
            .map(|field_env| format!("{}: Value", field_env.get_name()))
            .join(", ");
        emitln!(
            self.writer,
            "procedure {{:inline 1}} Pack_{}({}) returns (_struct: Value)\n{{",
            boogie_struct_name(struct_env),
            if !args_str.is_empty() && !type_args_str.is_empty() {
                format!("{}, {}", type_args_str, args_str)
            } else if args_str.is_empty() {
                type_args_str
            } else {
                args_str.clone()
            }
        );
        self.writer.indent();
        let mut fields_str = String::from("EmptyValueArray");
        for field_env in struct_env.get_fields() {
            let type_check = boogie_type_check(
                self.module_env.env,
                field_env.get_name().as_str(),
                &field_env.get_type(),
            );
            emit!(self.writer, &type_check);
            fields_str = format!("ExtendValueArray({}, {})", fields_str, field_env.get_name());
        }
        emitln!(self.writer, "_struct := Vector({});", fields_str);
        self.writer.unindent();
        emitln!(self.writer, "}\n");

        // Unpack function
        emitln!(
            self.writer,
            "procedure {{:inline 1}} Unpack_{}(_struct: Value) returns ({})\n{{",
            boogie_struct_name(struct_env),
            args_str
        );
        self.writer.indent();
        emitln!(self.writer, "assume is#Vector(_struct);");
        for field_env in struct_env.get_fields() {
            emitln!(
                self.writer,
                "{} := SelectField(_struct, {});",
                field_env.get_name(),
                boogie_field_name(&field_env)
            );
            let type_check = boogie_type_check(
                self.module_env.env,
                field_env.get_name().as_str(),
                &field_env.get_type(),
            );
            emit!(self.writer, &type_check);
        }
        self.writer.unindent();
        emitln!(self.writer, "}\n");
    }

    /// Translates all functions in the module.
    fn translate_functions(&self) {
        emitln!(
            self.writer,
            "\n\n// ** functions of module {}\n",
            self.module_env.get_id().name()
        );
        let mut num_fun_specified = 0;
        let mut num_fun = 0;
        for func_env in self.module_env.get_functions() {
            if !func_env.is_native() {
                num_fun += 1;
            }
            if !func_env.get_specification().is_empty() && !func_env.is_native() {
                num_fun_specified += 1;
            }
            self.writer
                .set_location(self.module_env.get_module_idx(), func_env.get_loc());
            self.translate_function(&func_env);
        }
        if num_fun > 0 {
            info!(
                "{} out of {} functions are specified in module {}",
                num_fun_specified,
                num_fun,
                self.module_env.get_id().name()
            );
        }
    }

    /// Translates the given function.
    fn translate_function(&self, func_env: &FunctionEnv<'_>) {
        if func_env.is_native() {
            if self.module_env.env.options.native_stubs {
                self.generate_function_sig(func_env, true);
                emit!(self.writer, ";");
                self.generate_function_spec(func_env);
                emitln!(self.writer);
            }
            return;
        }

        // generate inline function with function body
        self.generate_function_sig(func_env, true); // inlined version of function
        self.generate_function_spec(func_env);
        self.generate_inline_function_body(func_env);
        // generate function body
        emitln!(self.writer);

        // generate the _verify version of the function which calls inline version for standalone
        // verification.
        self.generate_function_sig(func_env, false); // no inline
        self.generate_verify_function_body(func_env); // function body just calls inlined version
    }

    /// Translates one bytecode instruction.
    fn translate_bytecode(
        &self,
        func_env: &FunctionEnv<'_>,
        offset: u16,
        bytecode: &StacklessBytecode,
    ) {
        // For debugging purposes, might temporarily activate this.
        // emitln!(self.writer, "        // {:?}", bytecode);

        self.writer.set_location(
            self.module_env.get_module_idx(),
            func_env.get_bytecode_loc(offset),
        );

        let propagate_abort = "if (__abort_flag) { goto Label_Abort; }";
        match bytecode {
            Branch(target) => emitln!(self.writer, "goto Label_{};", target),
            BrTrue(target, idx) => emitln!(
                self.writer,
                "__tmp := GetLocal(__m, __frame + {});\nif (b#Boolean(__tmp)) {{ goto Label_{}; }}",
                idx,
                target,
            ),
            BrFalse(target, idx) => emitln!(
                self.writer,
                "__tmp := GetLocal(__m, __frame + {});\nif (!b#Boolean(__tmp)) {{ goto Label_{}; }}",
                idx,
                target,
            ),
            MoveLoc(dest, src) => {
                if self.get_local_type(func_env, *dest).is_reference() {
                    emitln!(
                        self.writer,
                        "call __t{} := CopyOrMoveRef({});",
                        dest,
                        func_env.get_local_name(*src)
                    )
                } else {
                    emitln!(
                        self.writer,
                        "call __tmp := CopyOrMoveValue(GetLocal(__m, __frame + {}));",
                        src
                    );
                    emitln!(
                        self.writer,
                        "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                        dest
                    );
                }
            }
            CopyLoc(dest, src) => {
                if self.get_local_type(func_env, *dest).is_reference() {
                    emitln!(
                        self.writer,
                        "call __t{} := CopyOrMoveRef({});",
                        dest,
                        func_env.get_local_name(*src)
                    )
                } else {
                    emitln!(
                        self.writer,
                        "call __tmp := CopyOrMoveValue(GetLocal(__m, __frame + {}));",
                        src
                    );
                    emitln!(
                        self.writer,
                        "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                        dest
                    );
                }
            }
            StLoc(dest, src) => {
                if self.get_local_type(func_env, *dest as usize).is_reference() {
                    emitln!(
                        self.writer,
                        "call {} := CopyOrMoveRef(__t{});",
                        func_env.get_local_name(*dest),
                        src
                    )
                } else {
                    emitln!(
                        self.writer,
                        "call __tmp := CopyOrMoveValue(GetLocal(__m, __frame + {}));",
                        src
                    );
                    emitln!(
                        self.writer,
                        "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                        dest
                    );
                }
            }
            BorrowLoc(dest, src) => emitln!(
                self.writer,
                "call __t{} := BorrowLoc(__frame + {});",
                dest,
                src
            ),
            ReadRef(dest, src) => {
                emitln!(self.writer, "call __tmp := ReadRef(__t{});", src);
                emit!(
                    self.writer,
                    &boogie_type_check(
                        self.module_env.env,
                        "__tmp",
                        &self.get_local_type(func_env, *dest)
                    )
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            WriteRef(dest, src) => emitln!(
                self.writer,
                "call WriteRef(__t{}, GetLocal(__m, __frame + {}));",
                dest,
                src
            ),
            FreezeRef(dest, src) => emitln!(self.writer, "call __t{} := FreezeRef(__t{});", dest, src),
            Call(dests, callee_index, type_actuals, args) => {
                let (callee_module_env, callee_def_idx) =
                    self.module_env.get_callee_info(callee_index);
                let callee_env = callee_module_env.get_function(&callee_def_idx);
                let mut dest_str = String::new();
                let mut args_str = String::new();
                let mut dest_type_assumptions = vec![];
                let mut tmp_assignments = vec![];

                args_str.push_str(&boogie_type_values(
                    func_env.module_env.env,
                    &func_env.module_env.get_type_actuals(*type_actuals),
                ));
                if !args_str.is_empty() && !args.is_empty() {
                    args_str.push_str(", ");
                }
                args_str.push_str(
                    &args
                        .iter()
                        .map(|arg_idx| {
                            if self.get_local_type(func_env, *arg_idx).is_reference() {
                                format!("__t{}", arg_idx)
                            } else {
                                format!("GetLocal(__m, __frame + {})", arg_idx)
                            }
                        })
                        .join(", "),
                );
                dest_str.push_str(
                    &dests
                        .iter()
                        .map(|dest_idx| {
                            let dest = format!("__t{}", dest_idx);
                            let dest_type = &self.get_local_type(func_env, *dest_idx);
                            dest_type_assumptions.push(boogie_type_check(
                                self.module_env.env,
                                &dest,
                                dest_type,
                            ));
                            if !dest_type.is_reference() {
                                tmp_assignments.push(format!(
                                    "__m := UpdateLocal(__m, __frame + {}, __t{});",
                                    dest_idx, dest_idx
                                ));
                            }
                            dest
                        })
                        .join(", "),
                );
                if dest_str == "" {
                    emitln!(
                        self.writer,
                        "call {}({});",
                        boogie_function_name(&callee_env),
                        args_str
                    );
                } else {
                    emitln!(
                        self.writer,
                        "call {} := {}({});",
                        dest_str,
                        boogie_function_name(&callee_env),
                        args_str
                    );
                }
                emitln!(self.writer, propagate_abort);
                for s in &dest_type_assumptions {
                    emitln!(self.writer, s);
                }
                for s in &tmp_assignments {
                    emitln!(self.writer, s);
                }
            }
            Pack(dest, struct_def_index, type_actuals, fields) => {
                let struct_env = func_env.module_env.get_struct(struct_def_index);
                let args_str = func_env
                    .module_env
                    .get_type_actuals(*type_actuals)
                    .iter()
                    .map(|s| boogie_type_value(self.module_env.env, s))
                    .chain(
                        fields
                            .iter()
                            .map(|i| format!("GetLocal(__m, __frame + {})", i)),
                    )
                    .join(", ");
                emitln!(
                    self.writer,
                    "call __tmp := Pack_{}({});",
                    boogie_struct_name(&struct_env),
                    args_str
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Unpack(dests, struct_def_index, _, src) => {
                let struct_env = &func_env.module_env.get_struct(struct_def_index);
                let mut dests_str = String::new();
                let mut tmp_assignments = vec![];
                for dest in dests.iter() {
                    if !dests_str.is_empty() {
                        dests_str.push_str(", ");
                    }
                    let dest_str = &format!("__t{}", dest);
                    let dest_type = &self.get_local_type(func_env, *dest);
                    dests_str.push_str(dest_str);
                    if !dest_type.is_reference() {
                        tmp_assignments.push(format!(
                            "__m := UpdateLocal(__m, __frame + {}, __t{});",
                            dest, dest
                        ));
                    }
                }
                emitln!(
                    self.writer,
                    "call {} := Unpack_{}(GetLocal(__m, __frame + {}));",
                    dests_str,
                    boogie_struct_name(struct_env),
                    src
                );
                for s in &tmp_assignments {
                    emitln!(self.writer, s);
                }
            }
            BorrowField(dest, src, field_def_index) => {
                let struct_env = self.module_env.get_struct_of_field(field_def_index);
                let field_env = &struct_env.get_field(field_def_index);
                emitln!(
                    self.writer,
                    "call __t{} := BorrowField(__t{}, {});",
                    dest,
                    src,
                    boogie_field_name(field_env)
                );
            }
            Exists(dest, addr, struct_def_index, type_actuals) => {
                let resource_type = boogie_struct_type_value(
                    self.module_env.env,
                    self.module_env.get_module_idx(),
                    struct_def_index,
                    &self.module_env.get_type_actuals(*type_actuals),
                );
                emitln!(
                    self.writer,
                    "call __tmp := Exists(GetLocal(__m, __frame + {}), {});",
                    addr,
                    resource_type
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            BorrowGlobal(dest, addr, struct_def_index, type_actuals) => {
                let resource_type = boogie_struct_type_value(
                    self.module_env.env,
                    self.module_env.get_module_idx(),
                    struct_def_index,
                    &self.module_env.get_type_actuals(*type_actuals),
                );
                emitln!(
                    self.writer,
                    "call __t{} := BorrowGlobal(GetLocal(__m, __frame + {}), {});",
                    dest,
                    addr,
                    resource_type,
                );
                emitln!(self.writer, propagate_abort);
            }
            MoveToSender(src, struct_def_index, type_actuals) => {
                let resource_type = boogie_struct_type_value(
                    self.module_env.env,
                    self.module_env.get_module_idx(),
                    struct_def_index,
                    &self.module_env.get_type_actuals(*type_actuals),
                );
                emitln!(
                    self.writer,
                    "call MoveToSender({}, GetLocal(__m, __frame + {}));",
                    resource_type,
                    src,
                );
                emitln!(self.writer, propagate_abort);
            }
            MoveFrom(dest, src, struct_def_index, type_actuals) => {
                let resource_type = boogie_struct_type_value(
                    self.module_env.env,
                    self.module_env.get_module_idx(),
                    struct_def_index,
                    &self.module_env.get_type_actuals(*type_actuals),
                );
                emitln!(
                    self.writer,
                    "call __tmp := MoveFrom(GetLocal(__m, __frame + {}), {});",
                    src,
                    resource_type,
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
                emit!(
                    self.writer,
                    &boogie_type_check(
                        self.module_env.env,
                        &format!("__t{}", dest),
                        &self.get_local_type(func_env, *dest)
                    )
                );
                emitln!(self.writer, propagate_abort);
            }
            Ret(rets) => {
                for (i, r) in rets.iter().enumerate() {
                    if self.get_local_type(func_env, *r).is_reference() {
                        emitln!(self.writer, "__ret{} := __t{};", i, r);
                    } else {
                        emitln!(self.writer, "__ret{} := GetLocal(__m, __frame + {});", i, r);
                    }
                }
                emitln!(self.writer, "return;");
            }
            LdTrue(idx) => {
                emitln!(self.writer, "call __tmp := LdTrue();");
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            LdFalse(idx) => {
                emitln!(self.writer, "call __tmp := LdFalse();");
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            LdU8(idx, num) => {
                emitln!(self.writer, "call __tmp := LdConst({});", num);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            LdU64(idx, num) => {
                emitln!(self.writer, "call __tmp := LdConst({});", num);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            LdU128(idx, num) => {
                emitln!(self.writer, "call __tmp := LdConst({});", num);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            CastU8(dest, src) => {
                emitln!(
                    self.writer,
                    "call __tmp := CastU8(GetLocal(__m, __frame + {}));",
                    src
                );
                emitln!(self.writer, propagate_abort);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            CastU64(dest, src) => {
                emitln!(
                    self.writer,
                    "call __tmp := CastU64(GetLocal(__m, __frame + {}));",
                    src
                );
                emitln!(self.writer, propagate_abort);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            CastU128(dest, src) => {
                emitln!(
                    self.writer,
                    "call __tmp := CastU128(GetLocal(__m, __frame + {}));",
                    src
                );
                emitln!(self.writer, propagate_abort);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            LdAddr(idx, addr_idx) => {
                let addr_int = self.module_env.get_address(addr_idx);
                emitln!(self.writer, "call __tmp := LdAddr({});", addr_int);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            Not(dest, operand) => {
                emitln!(
                    self.writer,
                    "call __tmp := Not(GetLocal(__m, __frame + {}));",
                    operand
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Add(dest, op1, op2) => {
                let add_type = match self.get_local_type(func_env, *dest) {
                    GlobalType::U8 => "U8",
                    GlobalType::U64 => "U64",
                    GlobalType::U128 => "U128",
                    _ => unreachable!(),
                };
                emitln!(
                    self.writer,
                    "call __tmp := Add{}(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    add_type,
                    op1,
                    op2
                );
                emitln!(self.writer, propagate_abort);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Sub(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "call __tmp := Sub(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    op1,
                    op2
                );
                emitln!(self.writer, propagate_abort);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Mul(dest, op1, op2) => {
                let mul_type = match self.get_local_type(func_env, *dest) {
                    GlobalType::U8 => "U8",
                    GlobalType::U64 => "U64",
                    GlobalType::U128 => "U128",
                    _ => unreachable!(),
                };
                emitln!(
                    self.writer,
                    "call __tmp := Mul{}(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    mul_type,
                    op1,
                    op2
                );
                emitln!(self.writer, propagate_abort);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Div(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "call __tmp := Div(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    op1,
                    op2
                );
                emitln!(self.writer, propagate_abort);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Mod(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "call __tmp := Mod(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    op1,
                    op2
                );
                emitln!(self.writer, propagate_abort);
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Lt(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "call __tmp := Lt(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    op1,
                    op2
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Gt(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "call __tmp := Gt(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    op1,
                    op2
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Le(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "call __tmp := Le(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    op1,
                    op2
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Ge(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "call __tmp := Ge(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    op1,
                    op2
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Or(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "call __tmp := Or(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    op1,
                    op2
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            And(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "call __tmp := And(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {}));",
                    op1,
                    op2
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Eq(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "__tmp := Boolean(IsEqual(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {})));",
                    op1,
                    op2
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            Neq(dest, op1, op2) => {
                emitln!(
                    self.writer,
                    "__tmp := Boolean(!IsEqual(GetLocal(__m, __frame + {}), GetLocal(__m, __frame + {})));",
                    op1,
                    op2
                );
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    dest
                );
            }
            BitOr(_, _, _) | BitAnd(_, _, _) | Xor(_, _, _) => {
                emitln!(
                    self.writer,
                    "// bit operation not supported: {:?}",
                    bytecode
                );
            }
            Abort(_) => emitln!(self.writer, "goto Label_Abort;"),
            GetGasRemaining(idx) => {
                emitln!(self.writer, "call __tmp := GetGasRemaining();");
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            GetTxnSequenceNumber(idx) => {
                emitln!(self.writer, "call __tmp := GetTxnSequenceNumber();");
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            GetTxnPublicKey(idx) => {
                emitln!(self.writer, "call __tmp := GetTxnPublicKey();");
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            GetTxnSenderAddress(idx) => {
                emitln!(self.writer, "call __tmp := GetTxnSenderAddress();");
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            GetTxnMaxGasUnits(idx) => {
                emitln!(self.writer, "call __tmp := GetTxnMaxGasUnits();");
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            GetTxnGasUnitPrice(idx) => {
                emitln!(self.writer, "call __tmp := GetTxnGasUnitPrice();");
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, __tmp);",
                    idx
                );
            }
            _ => emitln!(self.writer, "// unimplemented instruction: {:?}", bytecode),
        }
        emitln!(self.writer);
    }

    /// Return a string for a boogie procedure header.
    /// if inline = true, add the inline attribute and use the plain function name
    /// for the procedure name. Also inject pre/post conditions if defined.
    /// Else, generate the function signature without the ":inline" attribute, and
    /// append _verify to the function name.
    fn generate_function_sig(&self, func_env: &FunctionEnv<'_>, inline: bool) {
        let (args, rets) = self.generate_function_args_and_returns(func_env);
        if inline {
            emit!(
                self.writer,
                "procedure {{:inline 1}} {} ({}) returns ({})",
                boogie_function_name(func_env),
                args,
                rets,
            )
        } else {
            emit!(
                self.writer,
                "procedure {}_verify ({}) returns ({})",
                boogie_function_name(func_env),
                args,
                rets
            )
        }
    }

    /// Generate boogie representation of function args and return args.
    fn generate_function_args_and_returns(&self, func_env: &FunctionEnv<'_>) -> (String, String) {
        let args = func_env
            .get_type_parameters()
            .iter()
            .map(|TypeParameter(ref i, _)| format!("{}: TypeValue", i))
            .chain(
                func_env
                    .get_parameters()
                    .iter()
                    .map(|Parameter(ref i, ref s)| format!("{}: {}", i, boogie_local_type(s))),
            )
            .join(", ");
        let rets = func_env
            .get_return_types()
            .iter()
            .enumerate()
            .map(|(i, ref s)| format!("__ret{}: {}", i, boogie_local_type(s)))
            .join(", ");
        (args, rets)
    }

    /// Return string for the function specification.
    fn generate_function_spec(&self, func_env: &FunctionEnv<'_>) {
        emitln!(self.writer);
        SpecTranslator::new(func_env, self.writer).translate();
    }

    /// Return string for body of verify function, which is just a call to the
    /// inline version of the function.
    fn generate_verify_function_body(&self, func_env: &FunctionEnv<'_>) {
        let args = func_env
            .get_type_parameters()
            .iter()
            .map(|TypeParameter(i, _)| i.as_str().to_string())
            .chain(
                func_env
                    .get_parameters()
                    .iter()
                    .map(|Parameter(i, _)| i.as_str().to_string()),
            )
            .join(", ");
        let rets = (0..func_env.get_return_types().len())
            .map(|i| format!("__ret{}", i))
            .join(", ");
        let assumptions = "    assume ExistsTxnSenderAccount(__m, __txn);\n";
        if rets.is_empty() {
            emit!(
                self.writer,
                "\n{{\n{}    call {}({});\n}}\n\n",
                assumptions,
                boogie_function_name(func_env),
                args
            )
        } else {
            emit!(
                self.writer,
                "\n{{\n{}    call {} := {}({});\n}}\n\n",
                assumptions,
                rets,
                boogie_function_name(func_env),
                args
            )
        }
    }

    /// This generates boogie code for everything after the function signature
    /// The function body is only generated for the "inline" version of the function.
    fn generate_inline_function_body(&self, func_env: &FunctionEnv<'_>) {
        // Be sure to set back location to the whole function definition as a default, otherwise
        // we may get unassigned code locations associated with condition locations.
        self.writer
            .set_location(self.module_env.get_module_idx(), func_env.get_loc());
        let code = &self.stackless_bytecode[func_env.get_def_idx().0 as usize];

        emitln!(self.writer, "{");
        self.writer.indent();

        // Generate local variable declarations. They need to appear first in boogie.
        emitln!(self.writer, "// declare local variables");
        let num_args = func_env.get_parameters().len();
        for i in num_args..code.local_types.len() {
            let local_name = func_env.get_local_name(i as u8);
            let local_type = &self.module_env.globalize_signature(&code.local_types[i]);
            emitln!(
                self.writer,
                "var {}: {}; // {}",
                local_name,
                boogie_local_type(local_type),
                boogie_type_value(self.module_env.env, local_type)
            );
        }
        emitln!(self.writer, "var __tmp: Value;");
        emitln!(self.writer, "var __frame: int;");
        emitln!(self.writer, "var __saved_m: Memory;");

        emitln!(self.writer, "\n// initialize function execution");
        emitln!(self.writer, "assume !__abort_flag;");
        emitln!(self.writer, "__saved_m := __m;");
        emitln!(self.writer, "__frame := __local_counter;");
        emitln!(
            self.writer,
            "__local_counter := __local_counter + {};",
            code.local_types.len()
        );

        emitln!(self.writer, "\n// process and type check arguments");
        for i in 0..num_args {
            let local_name = func_env.get_local_name(i as u8);
            let local_type = &self.module_env.globalize_signature(&code.local_types[i]);
            let type_check = boogie_type_check(self.module_env.env, &local_name, local_type);
            emit!(self.writer, &type_check);
            if !local_type.is_reference() {
                emitln!(
                    self.writer,
                    "__m := UpdateLocal(__m, __frame + {}, {});",
                    i,
                    local_name
                );
            }
        }
        // Local counter needs to be updated after type checks.

        emitln!(self.writer, "\n// bytecode translation starts here");

        // Identify all the branching targets so we can insert labels in front of them
        let mut branching_targets: BTreeSet<usize> = BTreeSet::new();
        for bytecode in code.code.iter() {
            match bytecode {
                Branch(target) | BrTrue(target, _) | BrFalse(target, _) => {
                    branching_targets.insert(*target as usize);
                }
                _ => {}
            }
        }

        // Generate bytecode
        for (offset, bytecode) in code.code.iter().enumerate() {
            // insert labels for branching targets
            if branching_targets.contains(&offset) {
                self.writer.unindent();
                emitln!(self.writer, "Label_{}:", offset);
                self.writer.indent();
            }
            self.translate_bytecode(func_env, offset as u16, bytecode);
        }

        // Generate abort exit.
        self.writer.unindent();
        emitln!(self.writer, "Label_Abort:");
        self.writer.indent();
        emitln!(self.writer, "__abort_flag := true;");
        emitln!(self.writer, "__m := __saved_m;");
        for (i, ref sig) in func_env.get_return_types().iter().enumerate() {
            if sig.is_reference() {
                emitln!(self.writer, "__ret{} := DefaultReference;", i);
            } else {
                emitln!(self.writer, "__ret{} := DefaultValue;", i);
            }
        }
        self.writer.unindent();
        emitln!(self.writer, "}");
    }

    /// Looks up the type of a local in the stackless bytecode representation.
    fn get_local_type(&self, func_env: &FunctionEnv<'_>, local_idx: usize) -> GlobalType {
        self.module_env.globalize_signature(
            &self.stackless_bytecode[func_env.get_def_idx().0 as usize].local_types[local_idx],
        )
    }
}
