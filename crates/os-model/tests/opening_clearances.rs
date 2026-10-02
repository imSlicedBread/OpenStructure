use os_core::Point2;
use os_model::{ClearanceEnd, OpeningClearance, WallPath};

#[test]
fn clearance_lock_math_uses_analytic_signed_arcs_and_rejects_invalid_distances() {
    for sweep in [2., -2., 5.6, -5.6] {
        let path = WallPath::CircularArc {
            center: Point2::new(4., -3.),
            radius: 5.,
            start_angle_rad: 0.7,
            signed_sweep_rad: sweep,
        };
        for width in [0.9, 1.5] {
            assert_eq!(
                OpeningClearance {
                    end: ClearanceEnd::Start,
                    distance: 0.4
                }
                .offset(path.length(), width)
                .unwrap(),
                0.4
            );
            assert_eq!(
                OpeningClearance {
                    end: ClearanceEnd::End,
                    distance: 0.4
                }
                .offset(path.length(), width)
                .unwrap(),
                5. * sweep.abs() - width - 0.4
            );
        }
    }
    for distance in [-1., f64::NAN, f64::INFINITY] {
        assert!(
            OpeningClearance {
                end: ClearanceEnd::End,
                distance
            }
            .offset(10., 1.)
            .is_err()
        );
    }
}
