use crate::parser::Expr;
use crate::parser::Stmt;
use crate::parser::Stmt::{CallCmd, DefCmd, DefFun, DefVar, Return};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::collections::HashSet;

const INDENT: isize = 4;

macro_rules! output {
    ($fs: expr, $indent: expr, $($arg:tt)*) => {
        write!($fs, "{}", (0..$indent).map(|_| " ").collect::<String>()).expect("write error");
        writeln!($fs, $($arg)*).expect("write error");
    }
}

pub fn gen_code(stmts: &Vec<Stmt>, sym_table: &HashSet<&String>, file: &str) {
    eprintln!("Generating rust code to {} ...", file);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o755)
        .open(file)
        .expect("create file for writing failed");

    output!(file, 0, "// Generated by rust-shell-script");
    output!(file, 0, "mod cmd_lib;");
    output!(file, 0, "use crate::cmd_lib::{{CmdResult, FunResult}};");
    output!(file, 0, "");

    for stmt in stmts {
        match stmt {
            DefFun(fun_name, parameters, body) => {
                visit_def_fun(sym_table, &mut file, &fun_name, &parameters, &body)
            }
            DefCmd(cmd_name, parameters, body) => {
                visit_def_cmd(sym_table, &mut file, &cmd_name, &parameters, &body)
            }
            _ => eprintln!("Not supported yet!"),
        }
    }
}

fn visit_def_var(file: &mut File, indent: &mut isize, var_name: &str, var_def: &Option<Expr>) {
    if let Some(expr) = var_def {
        output!(file, *indent, "let {} = {};", var_name, visit_expr(expr));
    } else {
        output!(file, *indent, "let {} = String::new();", var_name);
    }
}

fn visit_def_fun(sym_table: &HashSet<&String>, file: &mut File, fun_name: &str, parameters: &Vec<String>, body: &Vec<Stmt>) {
    let mut indent = 0;
    let mut fun_args = String::new();

    fun_args = format!("fn {}(", fun_name);
    for (i, p) in parameters.iter().enumerate() {
        if i != 0 {
            fun_args += ", ";
        }
        fun_args += format!("{}: &str", p).as_ref();
    }
    fun_args += ") -> FunResult {";
    output!(file, 0, "{}", fun_args);

    indent += INDENT;
    for (i, stmt) in body.iter().enumerate() {
        visit_stmt(sym_table, file, &mut indent, stmt, i == body.len()-1);
    }

    output!(file, 0, "}}");
    output!(file, 0, "");
}

fn visit_def_cmd(sym_table: &HashSet<&String>, file: &mut File, fun_name: &str, parameters: &Vec<String>, body: &Vec<Stmt>) {
    let mut indent = 0;
    let mut fun_args = String::new();

    fun_args = format!("fn {}(", fun_name);
    for (i, p) in parameters.iter().enumerate() {
        if i != 0 {
            fun_args += ", ";
        }
        fun_args += format!("{}: &str", p).as_ref();
    }
    fun_args += ") -> CmdResult {";
    output!(file, 0, "{}", fun_args);

    indent += INDENT;
    for (i, stmt) in body.iter().enumerate() {
        visit_stmt(sym_table, file, &mut indent, stmt, i == body.len()-1);
    }

    output!(file, 0, "}}");
    output!(file, 0, "");
}

fn visit_stmt(sym_table: &HashSet<&String>, file: &mut File, indent: &mut isize, stmt: &Stmt, is_last: bool) {
    match stmt {
        CallCmd(cmd, parameters) => visit_call_cmd(sym_table, file, indent, &cmd, &parameters, is_last),
        Return(expr) => visit_return(file, indent, &expr),
        DefVar(var_name, var_def) => visit_def_var(file, indent, &var_name, &var_def),
        _ => {
            let mut stmt = format!("{:?}", stmt);
            if !is_last {
                stmt += "?";
            }
            output!(file, *indent, "{}", stmt);
        }
    }
}

fn visit_call_cmd(sym_table: &HashSet<&String>, file: &mut File, indent: &mut isize, cmd: &str, parameters: &Vec<Expr>, is_last: bool) {
    let mut cmd = String::from(cmd);
    let mut ending = String::new();
    let mut builtin = false;

    if !is_last {
        if cmd == "info" {
            ending += ";";
        } else {
            ending += "?;";
        }
    }
    if cmd == "info" || cmd == "output" {
        cmd += "!";
        builtin = true;
    }

    if builtin || sym_table.contains(&cmd) {
        if parameters.len() == 0 {
            output!(file, *indent, "{}(){}", cmd, ending);
        } else {
            output!(file, *indent, "{}({}){}", cmd, visit_call(parameters), ending);
        }
    } else {
        if parameters.len() == 0 {
            output!(file, *indent, "run_cmd!(\"{}\"){}", cmd, ending);
        } else {
            output!(file, *indent, "run_cmd!(\"{} {}\"){}", cmd, visit_call(parameters), ending);
        }
    }
}
    
fn visit_call(parameters: &Vec<Expr>) -> String {
    let mut args = String::new();
    for (i, expr) in parameters.iter().enumerate() {
        if i > 0 {
            args += " ";
        }
        args += visit_expr(expr).as_ref();
    }
    format_str(&args)
}

fn visit_return(file: &mut File, indent: &mut isize, expr: &Expr) {
    output!(file, *indent, "return {}", visit_expr(expr));
}

fn visit_expr(expr: &Expr) -> String {
    match expr {
        Expr::LitNum(n) => {
            if *n == 0 {
                format!("Ok(())")
            } else {
                format!("Err(())")
            }
        },
        Expr::LitStr(s) => format!("\"{}\"", s),
        Expr::Var(v) => format!("\"${{{}}}\"", v.identifier),
        Expr::CallFun(f, args) => format!("{}({})?;", f, visit_call(args)),
        _ => format!("{:?}", expr),
    }
}

fn format_str(input: &str) -> String {
    let mut output = String::new();
    let mut vars = Vec::new();
    let mut input = input.chars().peekable();


    while let Some(c) = input.next() {
        if c == '$' && input.peek() == Some(&'{') {
            
            input.next();
            let mut var = String::new();
            while let Some(v) = input.next() {
                if v != '}' {
                    var.push(v);
                } else {
                    break;
                }
            }
            output += "{}";
            vars.push(var);
        } else {
            output.push(c);
        }
    }

    for v in vars {
        output += ", ";
        output += v.as_ref();
    }

    output
}

#[test]
fn test_format_str() {
    assert_eq!(format_str("${a} aa ${b} bb ${cc}"), "{} aa {} bb {}, a, b, cc".to_string());
}
