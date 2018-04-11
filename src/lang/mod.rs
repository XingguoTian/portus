use std;
use nom;

use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct Error(pub String);
impl std::error::Error for Error {
    fn description(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
impl From<String> for Error {
    fn from(e: String) -> Error {
        Error(e)
    }
}
impl<'a> From<&'a str> for Error {
    fn from(e: &'a str) -> Error {
        Error(String::from(e))
    }
}
impl From<nom::simple_errors::Err> for Error {
    fn from(e: nom::simple_errors::Err) -> Error {
        Error(String::from(e.description()))
    }
}
impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Error {
        Error(format!("err {}", e))
    }
}

mod ast;
mod datapath;
mod prog;
mod serialize;

pub use self::datapath::Bin;
pub use self::datapath::Reg;
pub use self::datapath::Scope;

use self::prog::Prog;

/// `compile()` uses 3 passes to yield Instrs.
///
/// 1. `Expr::new()` (called by `Prog::new_with_scope()` internally) returns a single AST
/// 2. `Prog::new_with_scope()` returns a list of ASTs for multiple expressions
/// 3. `Bin::compile_prog()` turns a `Prog` into a `Bin`, which is a `Vec` of datapath `Instr`
pub fn compile(src: &[u8]) -> Result<(Bin, Scope)> {
    Prog::new_with_scope(src).and_then(|(p, mut s)| Ok((Bin::compile_prog(&p, &mut s)?, s)))
}

/// `compile_and_serialize()` adds a fourth pass.
/// The resulting bytes can be passed to the datapath.
///
/// `serialize::serialize()` serializes a `Bin` into bytes.
pub fn compile_and_serialize(src: &[u8]) -> Result<(Vec<u8>, Scope)> {
    compile(src).and_then(|(b, s)| Ok((b.serialize()?, s)))
}

#[cfg(test)]
mod tests {
    extern crate test;
    use self::test::Bencher;
    
    #[bench]
    fn bench_1_line_compileonly(b: &mut Bencher) {
        let fold = "
            (def (foo 0))
            (:= Report.foo (+ Report.foo Ack.bytes_acked))
        ".as_bytes();
        b.iter(|| super::compile(fold).unwrap())
    }

    #[bench]
    fn bench_1_line(b: &mut Bencher) {
        let fold = "
            (def (foo 0))
            (:= Report.foo (+ Report.foo Ack.bytes_acked))
        ".as_bytes();
        b.iter(|| super::compile_and_serialize(fold).unwrap())
    }
    
    #[bench]
    fn bench_2_line(b: &mut Bencher) {
        let fold = "
            (def (foo 0) (bar 0))
            (:= Report.foo (+ Report.foo Ack.bytes_acked))
            (:= Report.bar (+ Report.bar Ack.bytes_misordered))
        ".as_bytes();
        b.iter(|| super::compile_and_serialize(fold).unwrap())
    }
    
    #[bench]
    fn bench_ewma(b: &mut Bencher) {
        let fold = "
            (def (foo 0) (bar 0))
            (:= Report.foo (+ Report.foo Ack.bytes_acked))
            (:= Report.bar (ewma 2 Flow.rate_outgoing))
        ".as_bytes();
        b.iter(|| super::compile_and_serialize(fold).unwrap())
    }
    
    #[bench]
    fn bench_if(b: &mut Bencher) {
        let fold = "
            (def (foo 0) (bar 0))
            (:= Report.foo (+ Report.foo Ack.bytes_acked))
            (bind isUrgent (!if isUrgent (> Ack.lost_pkts_sample 0)))
        ".as_bytes();
        b.iter(|| super::compile_and_serialize(fold).unwrap())
    }
    
    #[bench]
    fn bench_3_line(b: &mut Bencher) {
        let fold = "
            (def (foo 0) (bar 0) (baz 0))
            (:= Report.foo (+ Report.foo Ack.bytes_acked))
            (:= Report.bar (+ Report.bar Ack.bytes_misordered))
            (:= Report.baz (+ Report.bar Ack.ecn_bytes))
        ".as_bytes();
        b.iter(|| super::compile_and_serialize(fold).unwrap())
    }
}
