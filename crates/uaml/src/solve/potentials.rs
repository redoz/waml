//! Weighted union-find over per-axis coordinates. `pot[i]` is `coord[i]`
//! relative to the component root, so alignment/adjacency equalities compose.

pub struct Potentials {
    parent: Vec<usize>,
    pot: Vec<f64>,
}

impl Potentials {
    pub fn new(n: usize) -> Potentials {
        Potentials { parent: (0..n).collect(), pot: vec![0.0; n] }
    }

    /// Root of `i` and `coord[i] - coord[root]`, with path compression.
    pub fn find(&mut self, i: usize) -> (usize, f64) {
        let p = self.parent[i];
        if p == i {
            return (i, 0.0);
        }
        let (root, pr) = self.find(p);
        self.pot[i] += pr;
        self.parent[i] = root;
        (root, self.pot[i])
    }

    /// Enforce `coord[b] - coord[a] = delta`. `Err(existing)` if `a` and `b`
    /// are already related with a different delta (a contradiction).
    pub fn union(&mut self, a: usize, b: usize, delta: f64) -> Result<(), f64> {
        let (ra, da) = self.find(a);
        let (rb, db) = self.find(b);
        if ra == rb {
            let existing = db - da;
            if (existing - delta).abs() > 1e-6 {
                return Err(existing);
            }
            return Ok(());
        }
        self.parent[rb] = ra;
        self.pot[rb] = delta + da - db;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_composed_offsets() {
        let mut p = Potentials::new(3);
        p.union(0, 1, 10.0).unwrap(); // coord1 = coord0 + 10
        p.union(1, 2, 5.0).unwrap();  // coord2 = coord1 + 5
        let (r0, d0) = p.find(0);
        let (r2, d2) = p.find(2);
        assert_eq!(r0, r2);
        assert!((d2 - d0 - 15.0).abs() < 1e-9, "coord2 - coord0 == 15");
    }

    #[test]
    fn detects_contradiction() {
        let mut p = Potentials::new(2);
        p.union(0, 1, 10.0).unwrap();
        assert!(p.union(0, 1, 12.0).is_err());
        assert!(p.union(0, 1, 10.0).is_ok(), "consistent re-union is fine");
    }
}
