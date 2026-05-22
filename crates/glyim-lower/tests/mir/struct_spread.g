// ignore - requires full typeck/frontend (S03)
struct Point { x: i32, y: i32 }
fn main(base: Point) -> Point {
    Point { x: 1, ..base }
}
