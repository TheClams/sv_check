// This file is part of sv_check and subject to the terms of MIT Licence
// Copyright (c) 2019, clams@mail.com

use crate::error::{SvErrorKind, SvError, };
use crate::token::{TokenKind};
use crate::tokenizer::TokenStream;
use crate::ast::astnode::*;
use crate::ast::common::*;
use crate::ast::class::{parse_func,parse_task,parse_class_stmt_or_block};

// TODO
// - when parsing named block, ensure the name is unique

#[allow(dead_code)]
#[derive(PartialEq,Debug)]
pub enum ModuleCntxt {
    Top, Generate, Block, ForStmt, IfStmt
}


/// This function should be called after a keyword module/macromodule
pub fn parse_module_body(ts : &mut TokenStream, node : &mut AstNode, cntxt : ModuleCntxt) -> Result<(), SvError> {
    loop {
        let t = next_t!(ts,true);
        // println!("[parse_module_body] Token = {}", t);
        match t.kind {
            // Import statement
            TokenKind::KwImport => parse_import(ts,node)?,
            // Param/local param declaration
            TokenKind::KwParam | TokenKind::KwLParam => {
                ts.rewind(1); // put back the token so that it can be read by the parse param function
                // potential list of param (the parse function extract only one at a time)
                loop {
                    node.child.push(parse_param_decl(ts,true)?);
                    loop_args_break_cont!(ts,"parameter declaration",SemiColon);
                }
            }
            // Port
            TokenKind::KwInput | TokenKind::KwOutput | TokenKind::KwInout | TokenKind::KwRef => {
                ts.rewind(1); // put back the token so that it can be read by the parse param function
                // potential list of param (the parse function extract only one at a time)
                loop {
                    node.child.push(parse_port_decl(ts,false)?);
                    loop_args_break_cont!(ts,"parameter declaration",SemiColon);
                }

            }
            // Nettype
            TokenKind::KwNetType |
            TokenKind::KwSupply  =>  parse_signal_decl_list(ts,node)?,
            // Basetype
            TokenKind::KwReg         |
            TokenKind::TypeIntAtom   |
            TokenKind::TypeIntVector |
            TokenKind::TypeReal      |
            TokenKind::TypeString    |
            TokenKind::TypeCHandle   |
            TokenKind::TypeEvent     => parse_signal_decl_list(ts,node)?,
            TokenKind::KwEnum        => {
                let mut node_e = parse_enum(ts)?;
                let s = parse_ident_list(ts)?;
                node_e.attr.insert("name".to_string(),s);
                node.child.push(node_e);
            }
            TokenKind::KwTypedef => parse_typedef(ts,node)?,
            TokenKind::TypeGenvar => {
                ts.flush(0);
                let mut s = "".to_string();
                loop {
                    let mut nt = next_t!(ts,false);
                    if nt.kind!=TokenKind::Ident {
                        return Err(SvError::new(SvErrorKind::Syntax, nt.pos,
                                format!("Unexpected {} ({:?}) after genvar, expecting identifier",nt.value, nt.kind)));
                    }
                    s.push_str(&nt.value);
                    nt = next_t!(ts,false);
                    match nt.kind {
                        TokenKind::Comma => s.push_str(", "),
                        TokenKind::SemiColon => break,
                        _ => return Err(SvError::new(SvErrorKind::Syntax, nt.pos,
                                format!("Unexpected {} ({:?}) in genvar declaration, expecting , or ;",nt.value, nt.kind)))
                    }
                }
                node.child.push(AstNode::new(AstNodeKind::Genvar(s)));
            }
            // Identifier -> lookahead to detect if it is a signal declaration or an instantiation
            TokenKind::Ident => {
                let nt = next_t!(ts,true);
                // println!("[Module body] Ident followed by {}", nt.kind);
                match nt.kind {
                    // Scope -> this is a type definition
                    TokenKind::Scope => parse_signal_decl_list(ts,node)?,
                    //
                    TokenKind::Colon => {
                        ts.flush(2);
                        let mut n = AstNode::new(AstNodeKind::Statement);
                        n.attr.insert("label".to_string(),t.value);
                        // Expect assertion: not support for the moment ...
                        ts.skip_until(TokenKind::SemiColon)?;
                    }
                    // Identifier : could be a signal declaration or a module/interface instantiation
                    TokenKind::Ident => {
                        let nnt = next_t!(ts,true);
                        // println!("[Module body] (Ident Ident) followed by {}", nnt.kind);
                        match nnt.kind {
                            // Opening parenthesis indicates
                            // Semi colon or comma indicate signal declaration
                            TokenKind::SemiColon |
                            TokenKind::Comma     =>  parse_signal_decl_list(ts,node)?,
                            // Slice -> can be either an unpacked array declaration or an array of instance ...
                            // TODO: handle case of array of instances
                            TokenKind::SquareLeft =>  {
                                parse_signal_decl_list(ts,node)?;
                            }
                            // Open parenthesis -> instance
                            TokenKind::ParenLeft => {
                                let node_inst = parse_instance(ts)?;
                                node.child.push(node_inst);
                            }
                            _ => return Err(SvError::new(SvErrorKind::Syntax, t.pos,
                                    format!("Unexpected {} ({:?}) in signal declaration or instance",nnt.value, nnt.kind)))
                        }
                    }
                    // Dash is a clear indicator of an instance -> TODO
                    TokenKind::Hash => {
                        let node_inst = parse_instance(ts)?;
                        node.child.push(node_inst);
                    }
                    // Untreated token are forbidden
                    _ => return Err(SvError::new(SvErrorKind::Syntax, t.pos,
                            format!("Unexpected '{} {}' in signal declaration, expecting type or instance",t.value, nt.value)))
                }
            }
            TokenKind::KwBind => parse_bind(ts,node)?,
            //
            TokenKind::KwAssign | TokenKind::KwDefparam => {
                ts.rewind(1);
                node.child.push(parse_assign_c(ts)?);
            }
            // Always keyword
            TokenKind::KwAlways  |
            TokenKind::KwAlwaysC |
            TokenKind::KwAlwaysF |
            TokenKind::KwAlwaysL  => parse_always(ts, node)?,
            TokenKind::KwInitial  => parse_initial(ts, node)?,
            TokenKind::KwFunction => parse_func(ts, node, false, false)?,
            TokenKind::KwTask     => parse_task(ts, node)?,
            //
            TokenKind::KwTimeunit | TokenKind::KwTimeprec => parse_timescale(ts,node)?,
            //
            TokenKind::KwGenerate if cntxt==ModuleCntxt::Top => {
                ts.flush(0);
                parse_module_body(ts,node,ModuleCntxt::Generate)?;
            }
            TokenKind::KwFor  => parse_for(ts,node,true)?,
            TokenKind::KwIf   => {
                ts.flush(0);
                parse_if_else(ts,node, "if".to_string(), true)?;
            }
            TokenKind::KwBegin => {
                ts.flush(0);
                let mut n = AstNode::new(AstNodeKind::Block);
                parse_label(ts,&mut n,"block".to_string())?;
                parse_module_body(ts,&mut n, ModuleCntxt::Block)?;
                if n.attr["block"]!="" {
                    check_label(ts, &n.attr["block"])?;
                }
            }
            // End of loop depends on context
            TokenKind::KwEnd         if cntxt == ModuleCntxt::Block    => break,
            TokenKind::KwEndGenerate if cntxt == ModuleCntxt::Generate => break,
            TokenKind::KwEndModule   if cntxt == ModuleCntxt::Top      => break,
            TokenKind::Macro => parse_macro(ts,node)?,
            // Any un-treated token is an error
            _ => {
                // println!("{}", node);
                return Err(SvError::syntax(t, "module body".to_string()))
            }
        }

        if cntxt == ModuleCntxt::ForStmt || cntxt == ModuleCntxt::IfStmt {
            break;
        }
    }
    ts.flush(0);
    Ok(())
    // Err(SvError {kind:SvErrorKind::NotSupported, pos: t.pos, txt: "Module body".to_string()})
}

// Parse a continous assignment / defparam
pub fn parse_assign_c(ts : &mut TokenStream) -> Result<AstNode, SvError> {
    let mut node = AstNode::new(AstNodeKind::Assign);
    let t = next_t!(ts,false); // Get first word: expect assign or defparam
    node.attr.insert("kind".to_string(),t.value);
    // TODO: support drive/delay
    // t = next_t!(ts,true);
    node.child.push(parse_ident_hier(ts)?);
    expect_t!(ts,"continuous assignment",TokenKind::OpEq);
    let s = parse_expr(ts,ExprCntxt::Stmt)?;
    ts.flush(0); // Parse expression let the last character in the buffer -> this was a ;
    node.attr.insert("rhs".to_string(), s);
    // println!("[parse_assign_c] {}", node);
    Ok(node)
}

pub fn parse_assign_bnb(ts : &mut TokenStream) -> Result<AstNode, SvError> {
    let mut node = AstNode::new(AstNodeKind::Assign);
    // Parse LHS
    node.child.push(parse_ident_hier(ts)?);
    // Expect <=, = or composed asisgnement
    let mut t = expect_t!(ts,"assign",TokenKind::OpLTE,TokenKind::OpEq,TokenKind::OpCompAss);
    node.attr.insert("kind".to_string(),t.value);
    // Optional delay
    if t.kind==TokenKind::OpLTE {
        t = next_t!(ts,true);
        if t.kind==TokenKind::Hash {
            node.child.push(parse_delay(ts)?);
        } else {
            ts.rewind(1);
        }
    }
    //
    node.child.push(parse_class_expr(ts,ExprCntxt::Stmt,false)?);
    ts.flush(0); // consume the ;
    // println!("[parse_assign_c] {}", node);
    Ok(node)
}

// Parse an instance
#[allow(unused_assignments)]
pub fn parse_instance(ts : &mut TokenStream) -> Result<AstNode, SvError> {
    let mut node = AstNode::new(AstNodeKind::Instances);
    // First token is the module type
    ts.rewind(0);
    // ts.display_status("");
    let mut t = next_t!(ts,false);
    node.attr.insert("type".to_string(), t.value);
    t = next_t!(ts,true);
    parse_opt_params!(ts,node,t);
    ts.rewind(0);
    // Instances can be a list
    loop {
        t = expect_t!(ts,"instance name",TokenKind::Ident);
        let mut node_i = AstNode::new(AstNodeKind::Instance);
        node_i.attr.insert("name".to_string(), t.value);
        // Test for array of instance
        parse_opt_slice(ts, &mut node_i, false)?;
        parse_port_connection(ts,&mut node_i,false)?;
        node.child.push(node_i);
        loop_args_break_cont!(ts,"param declaration",SemiColon);
    }
    // println!("[Instance] {}",node);
    Ok(node)
}

// Parse an instance
#[allow(unused_assignments)]
pub fn parse_bind(ts : &mut TokenStream, node: &mut AstNode) -> Result<(), SvError> {
    ts.flush(1); // consume the bind keyword
    let mut n = AstNode::new(AstNodeKind::Bind);
    n.child.push(parse_ident_hier(ts)?); // TODO: handle variant of binding style
    n.child.push(parse_instance(ts)?);
    node.child.push(n);
    Ok(())
}

/// Parse an always block
pub fn parse_always(ts : &mut TokenStream, node: &mut AstNode) -> Result<(), SvError> {
    let t0 = next_t!(ts,false);
    let mut n = AstNode::new(AstNodeKind::Process);
    let mut is_block = false;
    let mut t = next_t!(ts,true);
    n.attr.insert("kind".to_string(),t0.value.clone());
    // println!("[parse_always] Node {}\nFirst Token {}",n, t);
    if t.kind == TokenKind::At {
        ts.flush(0);
        match t0.kind {
            TokenKind::KwAlwaysL |
            TokenKind::KwAlwaysC => return Err(SvError::new(SvErrorKind::Syntax, t.pos,
                                        format!("Sensitivity list not supported for {} process",t0.value))),
            // Parse the sensitivity list
            _ => {
                let node_s = parse_sensitivity(ts,true)?;
                n.child.push(node_s);
            }
        }

        t = next_t!(ts,true);
        // println!("[parse_always] Token post sensitivity list: {}", t);
    }
    if t.kind == TokenKind::KwBegin {
        is_block = true;
        parse_label(ts,&mut n,"block".to_string())?;
    }
    // Loop on statement, if/else / case
    parse_stmt(ts,&mut n, is_block)?;
    node.child.push(n);
    Ok(())
}

/// Parse sensitivity list
/// Suppose Token @ has been consumed
/// An empty sensitivity node corresponds to @(*) or @*
pub fn parse_sensitivity(ts : &mut TokenStream, is_process: bool) -> Result<AstNode, SvError> {
    let mut node = AstNode::new(AstNodeKind::Sensitivity);
    // Check next character: open parenthesis or *
    let mut t = next_t!(ts,false);
    // println!("[parse_sensitivity] First Token {}", t);
    match t.kind {
        TokenKind::OpStar   |
        TokenKind::SensiAll => return Ok(node),
        TokenKind::Ident if !is_process => {
            node.attr.insert("clk_event".to_string(), t.value);
            return Ok(node);
        }
        TokenKind::ParenLeft => {
            t = next_t!(ts,true);
            if t.kind == TokenKind::OpStar {
                ts.flush(1);
                expect_t!(ts,"sensitivity list",TokenKind::ParenRight);
                return Ok(node);
            }
        }
        _ => return Err(SvError::syntax(t, "sensitivity list. Expecting *, (*) or ( event list )".to_string()))
    }
    // Parse event list
    loop {
        // println!("[parse_sensitivity] First Token of event expression {}", t);
        // Capture optionnal edge
        let mut n = AstNode::new(AstNodeKind::Event);
        if t.kind == TokenKind::KwEdge {
            n.attr.insert("edge".to_string(),t.value );
            ts.flush(0); // consume keyword
        }
        // Capture event name
        n.child.push(parse_ident_hier(ts)?);
        // Check for iff
        t = next_t!(ts,false);
        if t.kind==TokenKind::KwIff {
            n.child.push(parse_class_expr(ts,ExprCntxt::Sensitivity,false)?);
            // n.child.push(parse_ident_hier(ts)?);
            t = next_t!(ts,false);
        }
        node.child.push(n);
        // Expecting closing parenthesis, comma, or keyword "or"
        // println!("[parse_sensitivity] event expression separator {}", t);
        match t.kind {
            TokenKind::ParenRight => break,
            TokenKind::KwOr  |
            TokenKind::Comma => {},
            _ => return Err(SvError::syntax(t, "sensitivity list. Expecting comma, keyword 'or' or )".to_string()))
        }
        t = next_t!(ts,true);
    }
    // println!("[parse_sensitivity] {}", node);
    Ok(node)
}

/// Parse an always block
pub fn parse_initial(ts : &mut TokenStream, node: &mut AstNode) -> Result<(), SvError> {
    let mut n = AstNode::new(AstNodeKind::Process);
    n.attr.insert("kind".to_string(),"initial".to_string());
    parse_class_stmt_or_block(ts,&mut n)?;
    node.child.push(n);
    Ok(())
}

///
pub fn parse_stmt_or_block(ts : &mut TokenStream, node: &mut AstNode) -> Result<(), SvError> {
    let mut is_block = false;
    let t = next_t!(ts,true);
    if t.kind == TokenKind::KwBegin {
        // println!("[parse_stmt_or_block] Parsing optional label");
        is_block = true;
        parse_label(ts,node,"block".to_string())?;
    }
    // Loop on statement, if/else / case
    parse_stmt(ts,node, is_block)?;
    Ok(())
}


/// Parse any statement in a process
pub fn parse_stmt(ts : &mut TokenStream, node: &mut AstNode, is_block: bool) -> Result<(), SvError> {
    ts.rewind(0);
    loop {
        let t = next_t!(ts,true);
        // println!("[parse_stmt] Token = {}", t);
        // ts.display_status("");
        match t.kind {
            TokenKind::KwIf   => {
                ts.flush(0);
                parse_if_else(ts,node, "if".to_string(), false)?;
            }
            TokenKind::KwCase     |
            TokenKind::KwPriority |
            TokenKind::KwUnique   |
            TokenKind::KwUnique0   => {
                ts.rewind(0);
                parse_case(ts,node)?;
            }
            TokenKind::KwFor  => parse_for(ts,node,false)?,
            TokenKind::Ident  => {
                ts.rewind(0);
                node.child.push(parse_assign_bnb(ts)?)
            },
            TokenKind::KwEnd if is_block => {
                ts.flush(0);
                break;
            },
            _ => return Err(SvError::new(SvErrorKind::Syntax, t.pos,
                            format!("Unexpected {} ({:?}) in statement",t.value, t.kind)))
        }
        // Stop parsing if not in a block
        if ! is_block {break;}
    }
    Ok(())
}

/// Parse If/Else if/Else statements
/// Suppose first IF has been consumed
pub fn parse_if_else(ts : &mut TokenStream, node: &mut AstNode, cond: String, is_gen: bool) -> Result<(), SvError> {
    let mut t = next_t!(ts,false);
    // println!("[parse_if_else] First Token = {}", t);
    // Parse IF condition
    if t.kind!=TokenKind::ParenLeft {
        return Err(SvError::syntax(t,"if. Expecting (".to_string()));
    }
    let mut node_if = AstNode::new(AstNodeKind::Branch);
    node_if.attr.insert("kind".to_string(),cond);
    let s = parse_expr(ts,ExprCntxt::Arg)?;
    // println!("[parse_if_else] Expr = {}", s);
    ts.flush(0); // No need to check last token, with this context parse expr only go out on close parenthesis
    node_if.attr.insert("expr".to_string(), s);
    // Check for begin
    let mut is_block = false;
    t = next_t!(ts,true);
    if t.kind == TokenKind::KwBegin {
        is_block = true;
        parse_label(ts,&mut node_if,"block".to_string())?;
    } else {
        ts.rewind(0);
    }
    // Loop on statement, if/else / case
    if is_gen {
        parse_module_body(ts,&mut node_if, if is_block {ModuleCntxt::Block} else {ModuleCntxt::IfStmt})?;
    } else {
        parse_stmt(ts,&mut node_if, is_block)?;
    }
    node.child.push(node_if);

    // Check for else if/else statement
    loop {
        t = next_t!(ts,true);
        // println!("[parse_if_else] Else Token ? {}", t);
        if t.kind == TokenKind::KwElse {
            ts.flush(0);
            t = next_t!(ts,true);
            // println!("[parse_if_else] If Token ? {}", t);
            if t.kind == TokenKind::KwIf {
                ts.flush(0);
                parse_if_else(ts,node,"else if".to_string(), is_gen)?;
            } else {
                let mut node_else = AstNode::new(AstNodeKind::Branch);
                node_else.attr.insert("kind".to_string(),"else".to_string());
                is_block = t.kind == TokenKind::KwBegin;
                // println!("[parse_if_else] Else token : is_block {}, is_gen {}", is_block, is_gen);
                if is_block {
                    parse_label(ts,&mut node_else,"block".to_string())?;
                }
                if is_gen {
                    ts.rewind(0);
                    parse_module_body(ts,&mut node_else, if is_block {ModuleCntxt::Block} else {ModuleCntxt::IfStmt})?;
                } else {
                    parse_stmt(ts,&mut node_else, is_block)?;
                }
                node.child.push(node_else);
                break;
            }
        }
        else {
            ts.rewind(0);
            break;
        }
    }
    Ok(())
}

/// Parse case statement
pub fn parse_case(ts : &mut TokenStream, node: &mut AstNode) -> Result<(), SvError> {
    ts.rewind(0);
    let mut t = next_t!(ts,false);
    let mut node_c = AstNode::new(AstNodeKind::Case);
    // println!("[parse_case] First Token {}", t);
    if t.kind==TokenKind::KwPriority || t.kind==TokenKind::KwUnique || t.kind==TokenKind::KwUnique0 {
        node_c.attr.insert("prio".to_string(),t.value);
        t = next_t!(ts,false);
    }
    if t.kind!=TokenKind::KwCase {
        return Err(SvError::new(SvErrorKind::Syntax, t.pos,
                    format!("Unexpected {} {:?} in case statement.",t.value, t.kind)));
    }
    node_c.attr.insert("kind".to_string(),t.value);
    // Parse case expression
    t = next_t!(ts,false);
    if t.kind!=TokenKind::ParenLeft {
        return Err(SvError::new(SvErrorKind::Syntax, t.pos,
                    format!("Expecting open parenthesis after if , got {} {:?}",t.value, t.kind)));
    }
    let s = parse_expr(ts,ExprCntxt::Arg)?;
    ts.flush(0); // consume closing parenthesis
    // println!("[parse_case] case expr {}", s.clone());
    node_c.attr.insert("expr".to_string(),s);
    // TODO: test for Match/inside
    t = next_t!(ts,true);
    match t.kind {
        TokenKind::KwUnique   |
        TokenKind::KwUnique0  |
        TokenKind::KwPriority => {
            ts.flush(0);
            node_c.attr.insert("prio".to_string(),t.value);
        }
        _ => ts.rewind(0)
    }
    // Loop on all case entry until endcase
    loop {
        t = next_t!(ts,true);
        // println!("[parse_case] case item {}", t);
        let mut node_i =  AstNode::new(AstNodeKind::CaseItem);
        match t.kind {
            TokenKind::OpPlus   |
            TokenKind::OpMinus  |
            TokenKind::Integer  |
            TokenKind::Ident    |
            TokenKind::KwTagged  => {
                ts.rewind(0);
                // Collect case item expression
                let mut s = "".to_string();
                loop {
                    let nt = next_t!(ts,false);
                    match nt.kind {
                        // TODO : be more restrictive, and also handle case like ranges
                        TokenKind::Colon => break,
                        _ => s.push_str(&nt.value),
                    }
                }
                // println!("[parse_case] case item value {}", s);
                node_i.attr.insert("item".to_string(),s);
                ts.flush(0); // every character until the colon should be consumed
            }
            TokenKind::KwDefault => {
                let nt = next_t!(ts,true);
                // Check for colon after keyword default
                if nt.kind!=TokenKind::Colon {
                    return Err(SvError::new(SvErrorKind::Syntax, nt.pos,
                                format!("Unexpected {} {:?} in default case item",nt.value, nt.kind)));
                }
                ts.flush(0);
                node_i.attr.insert("item".to_string(),"default".to_string());
            }
            TokenKind::KwEndcase => break,
            // TODO : support tagged keyword for case-matches
            _ => return Err(SvError::new(SvErrorKind::Syntax, t.pos,
                            format!("Unexpected {} ({:?}) in case entry",t.value, t.kind)))
        }
        // Parse statement
        parse_stmt_or_block(ts,&mut node_i)?;
        // println!("[parse_case] case item node {}", node_i);
        node_c.child.push(node_i);
    }
    ts.flush(0);
    node.child.push(node_c);
    Ok(())
}

pub fn parse_for(ts : &mut TokenStream, node: &mut AstNode, is_generate: bool) -> Result<(), SvError> {
    ts.flush(0);
    let mut t = next_t!(ts,false);
    if t.kind!=TokenKind::ParenLeft {
        return Err(SvError::syntax(t,"for. Expecting (".to_string()));
    }
    let mut node_for = AstNode::new(AstNodeKind::LoopFor);
    // Parse init part : end on ;
    let s = parse_expr(ts,ExprCntxt::Stmt)?;
    node_for.attr.insert("init".to_string(), s);
    ts.flush(0); // clear semi-colon
    // Parse test part : end on ;
    let s = parse_expr(ts,ExprCntxt::Stmt)?;
    node_for.attr.insert("test".to_string(), s);
    ts.flush(0); // clear semi-colon
    // Parse incr part : end on )
    let s = parse_expr(ts,ExprCntxt::Arg)?;
    node_for.attr.insert("incr".to_string(), s);
    ts.flush(0); // Clear parenthesis
    // TODO: analyze each statement to make sure those are valid
    // Check for begin
    let mut cntxt_body = ModuleCntxt::ForStmt;
    t = next_t!(ts,true);
    let is_block = t.kind == TokenKind::KwBegin;
    if is_block {
        cntxt_body = ModuleCntxt::Block;
        parse_label(ts,&mut node_for,"block".to_string())?;
    }
    // Parse content of for loop as if inside a module body
    if is_generate {
        parse_module_body(ts,&mut node_for,cntxt_body)?;
    } else {
        parse_stmt(ts,&mut node_for, is_block)?;
    }
    // println!("{}", node_for);
    node.child.push(node_for);
    Ok(())
}


pub fn parse_timescale(ts : &mut TokenStream, node: &mut AstNode) -> Result<(), SvError> {
    ts.rewind(0);
    let mut node_ts = AstNode::new(AstNodeKind::Timescale);
    let mut t = next_t!(ts,false);
    let allow_timeprec = t.kind==TokenKind::KwTimeunit;
    let mut time = parse_time(ts)?;
    node_ts.attr.insert(t.value, time);
    // Check if followed
    t = next_t!(ts,false);
    match t.kind {
        TokenKind::SemiColon => {}
        TokenKind::OpDiv if allow_timeprec => {
            time = parse_time(ts)?;
            node_ts.attr.insert("timeprecision".to_string(), time);
            t = next_t!(ts,false);
            if t.kind != TokenKind::SemiColon {
                return Err(SvError::syntax(t,"timescale. Expecting ;".to_string()));
            }
        }
        _ => return Err(SvError::syntax(t,"timescale. Expecting ; or /".to_string()))
    }
    node.child.push(node_ts);
    Ok(())
}


pub fn parse_time(ts : &mut TokenStream) -> Result<String,SvError> {
    let t1 = next_t!(ts,false);
    if t1.kind!=TokenKind::Integer && t1.kind!=TokenKind::Real {
        return Err(SvError::syntax(t1,"timescale. Expecting time value (integer or floating)".to_string()));
    }
    let t2 = next_t!(ts,false);
    if t2.kind!=TokenKind::Ident {
        return Err(SvError::syntax(t2,"timescale. Expecting time unit".to_string()));
    }
    match t2.value.as_ref() {
        "fs" |"ps" |"ns" |"us" |"ms" | "s" => {},
        _ => return Err(SvError::syntax(t2,"timescale. Expecting fs, ps, ns, ...".to_string()))
    }
    Ok(format!("{}{}",t1.value,t2.value))
}