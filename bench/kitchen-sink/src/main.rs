//! Benchmark crate that exercises many parts of rustc: proc-macro derives, async state
//! machines, generics + monomorphization, trait objects, macro_rules, closures, iterators,
//! pattern matching, const generics, and lifetimes.
use clap::Parser;
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_t = 1000)]
    n: usize,
    #[arg(long, default_value = "a+b")]
    pattern: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum Shape {
    Circle { r: f64 },
    Rect { w: f64, h: f64 },
    Poly(Vec<(f64, f64)>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Scene {
    name: String,
    shapes: Vec<Shape>,
    tags: BTreeMap<String, Vec<u32>>,
    meta: Option<HashMap<String, serde_json::Value>>,
}

trait Area {
    fn area(&self) -> f64;
}
impl Area for Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle { r } => std::f64::consts::PI * r * r,
            Shape::Rect { w, h } => w * h,
            Shape::Poly(p) => {
                let n = p.len();
                (0..n)
                    .map(|i| {
                        let (x1, y1) = p[i];
                        let (x2, y2) = p[(i + 1) % n];
                        x1 * y2 - x2 * y1
                    })
                    .sum::<f64>()
                    .abs()
                    / 2.0
            }
        }
    }
}

struct Matrix<const R: usize, const C: usize>([[f64; C]; R]);
impl<const R: usize, const K: usize> Matrix<R, K> {
    fn mul<const C: usize>(&self, o: &Matrix<K, C>) -> Matrix<R, C> {
        let mut out = [[0.0; C]; R];
        for i in 0..R {
            for j in 0..C {
                for k in 0..K {
                    out[i][j] += self.0[i][k] * o.0[k][j];
                }
            }
        }
        Matrix(out)
    }
}

macro_rules! gen_ops {
    ($($name:ident => $op:tt),* $(,)?) => {
        $(fn $name<T: std::ops::Add<Output = T> + std::ops::Mul<Output = T> + Copy>(a: T, b: T) -> T { a $op b })*
    };
}
gen_ops!(add_g => +, mul_g => *);

fn generic_stats<T, I>(it: I) -> (usize, Option<T>)
where
    T: PartialOrd + Copy + Debug,
    I: IntoIterator<Item = T>,
{
    let mut n = 0;
    let mut max: Option<T> = None;
    for x in it {
        n += 1;
        max = match max {
            Some(m) if m >= x => Some(m),
            _ => Some(x),
        };
    }
    (n, max)
}

struct Parser2<'a> {
    src: &'a str,
    pos: usize,
}
impl<'a> Parser2<'a> {
    fn tokens(&mut self) -> Vec<&'a str> {
        let mut out = vec![];
        while self.pos < self.src.len() {
            let rest = &self.src[self.pos..];
            let end = rest.find(' ').unwrap_or(rest.len());
            out.push(&rest[..end]);
            self.pos += end + 1;
        }
        out
    }
}

async fn worker(id: usize, tx: tokio::sync::mpsc::Sender<(usize, f64)>, shapes: Vec<Shape>) {
    let total: f64 = shapes.iter().map(|s| s.area()).sum();
    tokio::task::yield_now().await;
    let _ = tx.send((id, total)).await;
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let shapes: Vec<Shape> = (0..args.n)
        .map(|i| match i % 3 {
            0 => Shape::Circle { r: i as f64 },
            1 => Shape::Rect { w: i as f64, h: 2.0 },
            _ => Shape::Poly(vec![(0.0, 0.0), (i as f64, 0.0), (0.0, 1.0)]),
        })
        .collect();
    let scene = Scene {
        name: "bench".into(),
        shapes: shapes.clone(),
        tags: (0..10).map(|i| (format!("t{i}"), vec![i; 3])).collect(),
        meta: None,
    };
    let json = serde_json::to_string(&scene).unwrap();
    let back: Scene = serde_json::from_str(&json).unwrap();
    assert_eq!(back.shapes, scene.shapes);

    let par: f64 = shapes.par_iter().map(Area::area).sum();
    let dyns: Vec<Box<dyn Area>> = shapes.iter().cloned().map(|s| Box::new(s) as Box<dyn Area>).collect();
    let seq: f64 = dyns.iter().map(|d| d.area()).sum();

    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    for (id, chunk) in shapes.chunks(100).enumerate() {
        tokio::spawn(worker(id, tx.clone(), chunk.to_vec()));
    }
    drop(tx);
    let mut async_total = 0.0;
    while let Some((_, t)) = rx.recv().await {
        async_total += t;
    }

    let re = Regex::new(&args.pattern).unwrap();
    let hits = re.find_iter(&json).count();
    let m = Matrix([[1.0, 2.0], [3.0, 4.0]]).mul(&Matrix([[5.0], [6.0]]));
    let toks = Parser2 { src: "a b c d", pos: 0 }.tokens();
    println!(
        "{par:.1} {seq:.1} {async_total:.1} {hits} {} {} {:?} {:?} {:?}",
        add_g(1, 2),
        mul_g(3.0, 4.0),
        m.0,
        toks,
        generic_stats(shapes.iter().map(|s| s.area() as i64))
    );
}
