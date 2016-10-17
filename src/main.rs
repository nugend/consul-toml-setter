#![feature(question_mark)]
#![recursion_limit = "1024"]
#[macro_use]
extern crate slog;

#[macro_use]
extern crate slog_scope;
extern crate slog_term;

use slog::DrainExt;

#[macro_use]
extern crate error_chain;

mod errors { error_chain! { } }

use errors::*;

extern crate clap;
use clap::{App, Arg};

extern crate toml;
use toml::{Table, Value};

#[macro_use]
extern crate lazy_static;
extern crate regex;
use regex::{Regex, Captures};

use std::io::Read;
use std::collections::{BTreeMap, LinkedList};

fn load_toml(fnm:& str) -> Result<Table> {
  let mut f = std::fs::File::open(&fnm).chain_err(||"Could not open file")?;
  let mut buffer = String::new();
  f.read_to_string(&mut buffer).chain_err(||"Could not read file")?;
  let mut p = toml::Parser::new(&buffer);
  p.parse().ok_or_else(||p.errors[0].clone()).chain_err(||"Could not parse file")
}

fn validate_tomls<'a,I>(it:I) -> bool 
  where I:Iterator<Item=&'a(&'a str,Result<Table>)> 
  {
  let bad_tomls = it.filter_map(|v| 
    if let Err(ref e) = v.1 {
      let fnm = v.0;
      crit!("An error occurred for file {}: {}", fnm, e);
      Some(())
      } else { None }
    );
    bad_tomls.count() <= 0
  }


fn walk(mut map:BTreeMap<String,Value>,v:Value,p:String) -> BTreeMap<String,Value> {
  match v {
    Value::Table(t) => t.into_iter().fold(map,|acc,x| walk(acc,x.1,format!("{}/{}",p,x.0))),
    _ => {map.insert(p,v); map}
    }
  }

//fn sub_refs(m:BTreeMap<String,Value>) -> BTreeMap<String,Value>{
//  lazy_static! {
//        static ref envR: Regex = Regex::new(r"\$\((?P<name>.+?)\)").unwrap();
//        static ref varR: Regex = Regex::new(r"\$\{(?P<name>.+?)\}").unwrap();
//    }
//  let expanded_refs = HashSet<&str>::new();
//  for(k,v) in m {
//    for caps in r.captures_iter(
//    }
//  }


fn expand_envs(r:&str) -> String {
  lazy_static! {static ref RE: Regex = Regex::new(r"\$\{(?P<name>.+?)\}").unwrap();}
  RE.replace_all(r,|caps:& Captures| std::env::var(caps.name("name").unwrap()).unwrap())
  }

fn expand_refs(map:&BTreeMap<String,Value>,r:&str) -> String {
  lazy_static! {static ref RE: Regex = Regex::new(r"\$\{(?P<name>.+?)\}").unwrap();}
  RE.replace_all(r,|caps:& Captures| expand_ref(map,LinkedList::new(),caps.name("name").unwrap()).unwrap())
  }
fn expand_ref<'a>(map:&BTreeMap<String,Value>,mut seen:LinkedList<&'a str>,r:&'a str) -> Result<String> {
  lazy_static! {static ref RE: Regex = Regex::new(r"\$\{(?P<name>.+?)\}").unwrap();}
  if seen.contains(&r) {return Err(format!("Cycle detected in variable references {:#?}",seen).into())} else {seen.push_back(r)};
  let v = map.get(r).ok_or(format!("Invalid access to '{}'",r))?;
  match *v {
    Value::String(ref s)=> Ok(RE.replace_all(&s,|caps:& Captures| expand_ref(map,seen.clone(),caps.name("name").unwrap()).unwrap())),
    Value::Array(_) => Err(format!("Expansion requsted for '{}' which was an array",r).into()),
    Value::Table(_) => Err(format!("Expansion requested for '{}' which was a table",r).into()),
    _ => Ok(format!("{}",v))
    }
  }

fn main() {
  slog_scope::set_global_logger(slog::Logger::root(slog_term::streamer().build().fuse(), o![]));
  let res = App::new("numvals").arg(Arg::with_name("files")
        .min_values(1)
        .required(true)
    ).get_matches();
  let tomls: Vec<(&str,Result<Table>)> = res.values_of("files").unwrap().map(|fnm| (fnm,load_toml(&fnm))).collect();
  if !validate_tomls(tomls.iter()) { std::process::exit(1) }
  let flatmap = tomls.into_iter().fold(BTreeMap::new(),|acc,x|walk(acc,Value::Table(x.1.unwrap()),"".to_owned()));
  println!("{:#?}",flatmap);
}
