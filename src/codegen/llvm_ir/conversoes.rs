use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;

use super::LlvmGenerator;

impl<'a> LlvmGenerator<'a> {
    pub(crate) fn get_safe_string_ptr(&mut self, reg: &str) -> String {
        let is_null_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = icmp eq i8* {1}, null\n",
            is_null_reg, reg
        ));

        let empty_str_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [1 x i8], [1 x i8]* @.empty_str, i32 0, i32 0\n",
            empty_str_ptr
        ));

        let result_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = select i1 {1}, i8* {2}, i8* {3}\n",
            result_reg, is_null_reg, empty_str_ptr, reg
        ));
        result_reg
    }

    pub(crate) fn ensure_string(&mut self, reg: &str, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Texto => self.get_safe_string_ptr(reg),
            ast::Tipo::Decimal => self.get_safe_string_ptr(reg),
            ast::Tipo::Inteiro => self.convert_int_to_string(reg),
            ast::Tipo::Enum(_) => self.convert_int_to_string(reg),
            ast::Tipo::Flutuante => self.convert_float_to_string(reg),
            ast::Tipo::Duplo => self.convert_double_to_string(reg),
            ast::Tipo::Booleano => {
                let true_str = self.create_global_string("verdadeiro");
                let false_str = self.create_global_string("falso");
                let result_reg = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = select i1 {1}, i8* {2}, i8* {3}\n",
                    result_reg, reg, true_str, false_str
                ));
                result_reg
            }
            _ => self.create_global_string("[valor não textual]"),
        }
    }

    // Garante que o valor esteja no tipo esperado (apenas numéricos básicos por enquanto)
    pub(crate) fn ensure_value_type(&mut self, reg: &str, from: &ast::Tipo, to: &ast::Tipo) -> String {
        use ast::Tipo::*;
        match (from, to) {
            (f, t) if f == t => reg.to_string(),
            (Inteiro, Flutuante) => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = sitofp i32 {1} to float\n", tmp, reg));
                tmp
            }
            (Inteiro, Duplo) => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = sitofp i32 {1} to double\n", tmp, reg));
                tmp
            }
            (Flutuante, Duplo) => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = fpext float {1} to double\n", tmp, reg));
                tmp
            }
            (Duplo, Flutuante) => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = fptrunc double {1} to float\n", tmp, reg));
                tmp
            }
            _ => reg.to_string(),
        }
    }

    pub(crate) fn convert_float_to_string(&mut self, f_reg: &str) -> String {
        let buffer = self.get_unique_temp_name();
        self.body
            .push_str(&format!("  {0} = alloca [64 x i8], align 1\n", buffer));
        let buffer_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [64 x i8], [64 x i8]* {1}, i32 0, i32 0\n",
            buffer_ptr, buffer
        ));
        let fmt_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [3 x i8], [3 x i8]* @.float_fmt, i32 0, i32 0\n",
            fmt_ptr
        ));
        let as_double = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = fpext float {1} to double\n",
            as_double, f_reg
        ));
        self.body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {0}, i8* {1}, double {2})\n",
            buffer_ptr, fmt_ptr, as_double
        ));
        buffer_ptr
    }

    pub(crate) fn convert_double_to_string(&mut self, d_reg: &str) -> String {
        let buffer = self.get_unique_temp_name();
        self.body
            .push_str(&format!("  {0} = alloca [64 x i8], align 1\n", buffer));
        let buffer_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [64 x i8], [64 x i8]* {1}, i32 0, i32 0\n",
            buffer_ptr, buffer
        ));
        let fmt_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [3 x i8], [3 x i8]* @.double_fmt, i32 0, i32 0\n",
            fmt_ptr
        ));
        self.body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {0}, i8* {1}, double {2})\n",
            buffer_ptr, fmt_ptr, d_reg
        ));
        buffer_ptr
    }

    pub(crate) fn ensure_float(&mut self, reg: &str, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Flutuante => reg.to_string(),
            ast::Tipo::Inteiro => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = sitofp i32 {1} to float\n", tmp, reg));
                tmp
            }
            ast::Tipo::Duplo => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = fptrunc double {1} to float\n", tmp, reg));
                tmp
            }
            _ => panic!("Conversão para float não suportada: {:?}", tipo),
        }
    }

    pub(crate) fn ensure_double(&mut self, reg: &str, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Duplo => reg.to_string(),
            ast::Tipo::Inteiro => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = sitofp i32 {1} to double\n", tmp, reg));
                tmp
            }
            ast::Tipo::Flutuante => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = fpext float {1} to double\n", tmp, reg));
                tmp
            }
            _ => panic!("Conversão para double não suportada: {:?}", tipo),
        }
    }

    pub(crate) fn ensure_bool(&mut self, reg: &str, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Booleano => reg.to_string(),
            ast::Tipo::Inteiro => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = icmp ne i32 {1}, 0\n", tmp, reg));
                tmp
            }
            ast::Tipo::Enum(_) => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = icmp ne i32 {1}, 0\n", tmp, reg));
                tmp
            }
            _ => panic!("Conversão para bool não suportada: {:?}", tipo),
        }
    }

    pub(crate) fn convert_int_to_string(&mut self, int_reg: &str) -> String {
        let buffer = self.get_unique_temp_name();
        self.body
            .push_str(&format!("  {0} = alloca [21 x i8], align 1\n", buffer));
        let buffer_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [21 x i8], [21 x i8]* {1}, i32 0, i32 0\n",
            buffer_ptr, buffer
        ));

        let format_specifier_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [3 x i8], [3 x i8]* @.int_fmt, i32 0, i32 0\n",
            format_specifier_ptr
        ));

        self.body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {0}, i8* {1}, i32 {2})\n",
            buffer_ptr, format_specifier_ptr, int_reg
        ));
        buffer_ptr
    }

    pub(crate) fn concatenate_strings(&mut self, str1_reg: &str, str2_reg: &str) -> String {
        let safe_str1 = self.get_safe_string_ptr(str1_reg);
        let safe_str2 = self.get_safe_string_ptr(str2_reg);

        let len1_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = call i64 @strlen(i8* {1})\n",
            len1_reg, safe_str1
        ));

        let len2_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = call i64 @strlen(i8* {1})\n",
            len2_reg, safe_str2
        ));

        let total_len_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = add i64 {1}, {2}\n",
            total_len_reg, len1_reg, len2_reg
        ));

        let alloc_size_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = add i64 {1}, 1\n",
            alloc_size_reg, total_len_reg
        ));

        let buffer_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = call i8* @malloc(i64 {1})\n",
            buffer_reg, alloc_size_reg
        ));

        let dest_ptr1 = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr i8, i8* {1}, i64 0\n",
            dest_ptr1, buffer_reg
        ));
        self.body.push_str(&format!("  call void @llvm.memcpy.p0i8.p0i8.i64(i8* align 1 {0}, i8* align 1 {1}, i64 {2}, i1 false)\n", dest_ptr1, safe_str1, len1_reg));

        let dest_ptr2 = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr i8, i8* {1}, i64 {2}\n",
            dest_ptr2, buffer_reg, len1_reg
        ));
        self.body.push_str(&format!("  call void @llvm.memcpy.p0i8.p0i8.i64(i8* align 1 {0}, i8* align 1 {1}, i64 {2}, i1 false)\n", dest_ptr2, safe_str2, len2_reg));

        let null_terminator_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr i8, i8* {1}, i64 {2}\n",
            null_terminator_ptr, buffer_reg, total_len_reg
        ));
        self.body
            .push_str(&format!("  store i8 0, i8* {0}\n", null_terminator_ptr));

        buffer_reg
    }

    pub(crate) fn create_global_string(&mut self, text: &str) -> String {
        let str_len = text.len() + 1;
        let str_name = format!("@.str.{0}", self.string_counter);
        self.string_counter += 1;
        let sanitized_text = text
            .replace('\\', "\\")
            .replace('\n', "\0A")
            .replace('"', "\"");
        self.header.push_str(&format!(
            "{0} = private unnamed_addr constant [{1} x i8] c\"{2}\\00\", align 1\n",
            str_name, str_len, sanitized_text
        ));

        let ptr_register = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [{1} x i8], [{1} x i8]* {2}, i32 0, i32 0\n",
            ptr_register, str_len, str_name
        ));
        ptr_register
    }

}
