//! `ConjGrad` de `gaussian.cpp`: gradiente conjugado sobre la matriz de contactos.

/// Resuelve `A x = b` (`a` por filas, `n × n`). Devuelve `x` y `res = |r| − tol` de la
/// última iteración, que `ProcessBodyColls3` usa para descartar sistemas sin converger.
pub fn conj_grad(a: &[f32], b: &[f32], n: usize, tol: f32, max_its: usize) -> (Vec<f32>, f32) {
    let mut x = vec![0.0f32; n];
    let mut p = b.to_vec();
    let mut r = b.to_vec();
    let mut t = vec![0.0f32; n];
    let mut r_sq = 0.0f32;
    let mut res;
    let mut its = 0usize;

    let mat_mul = |v: &[f32], out: &mut [f32]| {
        for i in 0..n {
            out[i] = (0..n).map(|j| a[i * n + j] * v[j]).sum();
        }
    };
    let dot = |u: &[f32], v: &[f32]| u.iter().zip(v).map(|(a, b)| a * b).sum::<f32>();

    loop {
        let r_sq_old = r_sq;
        r_sq = dot(&r, &r);
        let r_norm = r_sq.sqrt();
        res = r_norm - tol;
        if !(its < max_its && res > 0.0) {
            break;
        }
        if its > 0 {
            let beta = r_sq / r_sq_old;
            for i in 0..n {
                p[i] = r[i] + beta * p[i];
            }
        }
        mat_mul(&p, &mut t);
        let p_dot_t = dot(&p, &t);
        if p_dot_t.abs() > super::units::SMALL_REAL {
            let alpha = r_sq / p_dot_t;
            for i in 0..n {
                x[i] += alpha * p[i];
            }
            mat_mul(&p, &mut t);
            for i in 0..n {
                r[i] -= alpha * t[i];
            }
            its += 1;
        } else {
            break;
        }
    }
    (x, res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_symmetric_positive_system() {
        let a = [4.0, 1.0, 1.0, 3.0];
        let b = [1.0, 2.0];
        let (x, res) = conj_grad(&a, &b, 2, 1e-4, 10);
        assert!(res <= 0.0, "no convergió: {res}");
        assert!((x[0] - 1.0 / 11.0).abs() < 1e-3 && (x[1] - 7.0 / 11.0).abs() < 1e-3, "{x:?}");
    }
}
