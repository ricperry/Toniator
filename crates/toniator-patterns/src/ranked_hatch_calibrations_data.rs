//! Genuine ranked-screen and discrete hatch measurements from the ordinary engine.
//! Provenance: resources/ranked-hatch-calibration-provenance.json; no runtime fitting.
use super::{CalibrationKnot, MeasuredData};

/// Returns only the frozen complete current binding and its actually measured numeric payload.
pub(super) fn data(id: &str) -> Option<MeasuredData<'static>> {
    Some(match id {
        "concentric-hatching" => (
            "CalibrationBinding { recipe: PatternDefinitionRecipe { structure: OrderedOutputs { definition: GenericGuides { name: \"\", coverage: CoveragePolicy { guard_steps: 2, additional_margin: 0.0 }, dimensions: [GenericGuideDimensionDraft { baseline_angle_degrees: 0.0, phase: 0.0, prototype: CircularArc { center: AuthoredPoint2 { x: 0.0, y: 0.0 }, radius: 2.56, start_angle_degrees: 0.0, sweep_angle_degrees: 360.0 }, repetition: NormalOffset { spacing: 2.56, cleanup: DissolveCrossings } }], product: AlongGuides { dimension_indices: [0], interval_multiplier: 1.0, phase: 0.0 }, orientation: Fixed }, outputs: [StructuralPaths { tone_sample_spacing: ToneSampleSpacing { multiplier: 1.0 }, style: PathStrokeStyle { join: Round, cap: Round, hatch: HatchStyle { width: 1.0 } } }] }, output_settings: [PatternOutputSettingsRecipe { source_filter: All, response: Connected(ConnectedGeometryResponse { minimum_thickness: 0.0, maximum_thickness: 1.0, bias: 0.0 }) }] }, density_aspect: 1.0, shape_rotation_degrees: 0.0 }",
            "hatch-reference-fixed-canvas1024-spacings32-levels8-v1+anchored-tone-v1+interleaved-hatch-v3",
            &[
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.0,
                },
                CalibrationKnot {
                    input: 0.25,
                    coverage: 0.12587780258112025,
                },
                CalibrationKnot {
                    input: 0.5,
                    coverage: 0.2502596540515034,
                },
                CalibrationKnot {
                    input: 0.75,
                    coverage: 0.3765049424593184,
                },
                CalibrationKnot {
                    input: 1.0,
                    coverage: 0.4998700239893982,
                },
                CalibrationKnot {
                    input: 1.25,
                    coverage: 0.6272108117304708,
                },
                CalibrationKnot {
                    input: 1.5,
                    coverage: 0.7508982999671415,
                },
                CalibrationKnot {
                    input: 1.75,
                    coverage: 0.8761471661259034,
                },
                CalibrationKnot {
                    input: 2.0,
                    coverage: 0.9999970254032821,
                },
            ],
            None,
            None,
        ),
        "parallel-hatching" => (
            "CalibrationBinding { recipe: PatternDefinitionRecipe { structure: OrderedOutputs { definition: GeneralizedStraightGuides { name: \"\", coverage: CoveragePolicy { guard_steps: 2, additional_margin: 0.0 }, dimensions: [GuideDimensionDraft { baseline_angle_degrees: 0.0, phase: 0.0, spacing_multiplier: 0.25 }], product: AlongGuides { dimension_indices: [0], interval_multiplier: 1.0, phase: 0.0 }, orientation: Fixed }, outputs: [StructuralPaths { tone_sample_spacing: ToneSampleSpacing { multiplier: 1.0 }, style: PathStrokeStyle { join: Round, cap: Round, hatch: HatchStyle { width: 1.0 } } }] }, output_settings: [PatternOutputSettingsRecipe { source_filter: All, response: Connected(ConnectedGeometryResponse { minimum_thickness: 0.0, maximum_thickness: 1.0, bias: 0.0 }) }] }, density_aspect: 1.0, shape_rotation_degrees: 0.0 }",
            "hatch-reference-fixed-canvas1024-spacings32-levels8-v1+anchored-tone-v1+interleaved-hatch-v3",
            &[
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.0,
                },
                CalibrationKnot {
                    input: 0.25,
                    coverage: 0.125,
                },
                CalibrationKnot {
                    input: 0.5,
                    coverage: 0.25,
                },
                CalibrationKnot {
                    input: 0.75,
                    coverage: 0.375,
                },
                CalibrationKnot {
                    input: 1.0,
                    coverage: 0.5,
                },
                CalibrationKnot {
                    input: 1.25,
                    coverage: 0.625,
                },
                CalibrationKnot {
                    input: 1.5,
                    coverage: 0.75,
                },
                CalibrationKnot {
                    input: 1.75,
                    coverage: 0.875,
                },
                CalibrationKnot {
                    input: 2.0,
                    coverage: 1.0,
                },
            ],
            None,
            None,
        ),
        "ranked-blue-noise-marks" => (
            "CalibrationBinding { recipe: PatternDefinitionRecipe { structure: OrderedOutputs { definition: RandomSites { name: \"\", coverage: CoveragePolicy { guard_steps: 2, additional_margin: 0.0 }, character: Dispersion { layout: RankedBlueNoise }, nominal_spacing: Some(10.24), seed: 43, density_modulation: Uniform, exclusion: None, maximum_attempts: 16000000, maximum_neighbor_checks: 16000000, refinement: RandomSiteRefinement { enabled: false, density_weighted: false, iterations: 4 } }, outputs: [Marks] }, output_settings: [PatternOutputSettingsRecipe { source_filter: All, response: Marks(MarkGeometryResponse { minimum_fill: 0.0, maximum_fill: 1.0 }) }] }, density_aspect: 1.0, shape_rotation_degrees: 0.0 }",
            "ton018-reference-fixed-canvas1024-spacings32-seed-neutral-v5",
            &[
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.0,
                },
                CalibrationKnot {
                    input: 0.001,
                    coverage: 9.99992133632e-07,
                },
                CalibrationKnot {
                    input: 0.003,
                    coverage: 1.2725428248576e-05,
                },
                CalibrationKnot {
                    input: 0.004,
                    coverage: 2.2622983553024e-05,
                },
                CalibrationKnot {
                    input: 0.005,
                    coverage: 3.7528253394944e-05,
                },
                CalibrationKnot {
                    input: 0.00625,
                    coverage: 5.8638057709568e-05,
                },
                CalibrationKnot {
                    input: 0.0075,
                    coverage: 8.4438616195072e-05,
                },
                CalibrationKnot {
                    input: 0.00875,
                    coverage: 0.00011493016353792,
                },
                CalibrationKnot {
                    input: 0.01,
                    coverage: 0.000153076823887872,
                },
                CalibrationKnot {
                    input: 0.0125,
                    coverage: 0.000239265587888128,
                },
                CalibrationKnot {
                    input: 0.015,
                    coverage: 0.000344542347505664,
                },
                CalibrationKnot {
                    input: 0.02,
                    coverage: 0.000618097906188288,
                },
                CalibrationKnot {
                    input: 0.025,
                    coverage: 0.000965778721441792,
                },
                CalibrationKnot {
                    input: 0.030000000000000002,
                    coverage: 0.001397726032539648,
                },
                CalibrationKnot {
                    input: 0.035,
                    coverage: 0.001902460683350016,
                },
                CalibrationKnot {
                    input: 0.04,
                    coverage: 0.002492405822619648,
                },
                CalibrationKnot {
                    input: 0.05,
                    coverage: 0.003902111350280192,
                },
                CalibrationKnot {
                    input: 0.0625,
                    coverage: 0.006097050245414912,
                },
                CalibrationKnot {
                    input: 0.07500000000000001,
                    coverage: 0.008791657266212864,
                },
                CalibrationKnot {
                    input: 0.08750000000000001,
                    coverage: 0.011977731449389056,
                },
                CalibrationKnot {
                    input: 0.1,
                    coverage: 0.015644676527927295,
                },
                CalibrationKnot {
                    input: 0.109375,
                    coverage: 0.0187289909030912,
                },
                CalibrationKnot {
                    input: 0.11875000000000001,
                    coverage: 0.022077272750862336,
                },
                CalibrationKnot {
                    input: 0.12812500000000002,
                    coverage: 0.025714456129404928,
                },
                CalibrationKnot {
                    input: 0.1375,
                    coverage: 0.029615219662127105,
                },
                CalibrationKnot {
                    input: 0.146875,
                    coverage: 0.0338055278770176,
                },
                CalibrationKnot {
                    input: 0.15625,
                    coverage: 0.038258857874898945,
                },
                CalibrationKnot {
                    input: 0.165625,
                    coverage: 0.04298765676398592,
                },
                CalibrationKnot {
                    input: 0.175,
                    coverage: 0.04800784577534771,
                },
                CalibrationKnot {
                    input: 0.18437499999999998,
                    coverage: 0.05328984174986855,
                },
                CalibrationKnot {
                    input: 0.19374999999999998,
                    coverage: 0.05884692624751821,
                },
                CalibrationKnot {
                    input: 0.203125,
                    coverage: 0.06469700858361856,
                },
                CalibrationKnot {
                    input: 0.2125,
                    coverage: 0.07080684818538087,
                },
                CalibrationKnot {
                    input: 0.221875,
                    coverage: 0.07719232835706266,
                },
                CalibrationKnot {
                    input: 0.23125,
                    coverage: 0.08387186304322355,
                },
                CalibrationKnot {
                    input: 0.240625,
                    coverage: 0.09081012903183565,
                },
                CalibrationKnot {
                    input: 0.25,
                    coverage: 0.09802408628902708,
                },
                CalibrationKnot {
                    input: 0.2578125,
                    coverage: 0.10424632253366271,
                },
                CalibrationKnot {
                    input: 0.265625,
                    coverage: 0.11068017950042931,
                },
                CalibrationKnot {
                    input: 0.28125,
                    coverage: 0.1240843559731118,
                },
                CalibrationKnot {
                    input: 0.2890625,
                    coverage: 0.13107367142091367,
                },
                CalibrationKnot {
                    input: 0.296875,
                    coverage: 0.13827606487231284,
                },
                CalibrationKnot {
                    input: 0.3125,
                    coverage: 0.15321447319895654,
                },
                CalibrationKnot {
                    input: 0.3203125,
                    coverage: 0.16097122768604366,
                },
                CalibrationKnot {
                    input: 0.328125,
                    coverage: 0.1689410888185979,
                },
                CalibrationKnot {
                    input: 0.34375,
                    coverage: 0.18541379677473382,
                },
                CalibrationKnot {
                    input: 0.3515625,
                    coverage: 0.1939374748926587,
                },
                CalibrationKnot {
                    input: 0.359375,
                    coverage: 0.2026755939542057,
                },
                CalibrationKnot {
                    input: 0.375,
                    coverage: 0.22068268840012187,
                },
                CalibrationKnot {
                    input: 0.390625,
                    coverage: 0.23945604543209062,
                },
                CalibrationKnot {
                    input: 0.3984375,
                    coverage: 0.24913006497408616,
                },
                CalibrationKnot {
                    input: 0.40625,
                    coverage: 0.25904165802676227,
                },
                CalibrationKnot {
                    input: 0.421875,
                    coverage: 0.27935111814222235,
                },
                CalibrationKnot {
                    input: 0.4375,
                    coverage: 0.30042699451805693,
                },
                CalibrationKnot {
                    input: 0.453125,
                    coverage: 0.32226927747642165,
                },
                CalibrationKnot {
                    input: 0.46875,
                    coverage: 0.344877936473559,
                },
                CalibrationKnot {
                    input: 0.484375,
                    coverage: 0.3682529885460808,
                },
                CalibrationKnot {
                    input: 0.5,
                    coverage: 0.39244808334791886,
                },
                CalibrationKnot {
                    input: 0.515625,
                    coverage: 0.41735882702904525,
                },
                CalibrationKnot {
                    input: 0.53125,
                    coverage: 0.44303653749682587,
                },
                CalibrationKnot {
                    input: 0.546875,
                    coverage: 0.46948076704352254,
                },
                CalibrationKnot {
                    input: 0.5546875,
                    coverage: 0.4829903331156419,
                },
                CalibrationKnot {
                    input: 0.5625,
                    coverage: 0.4967410014045962,
                },
                CalibrationKnot {
                    input: 0.578125,
                    coverage: 0.5247210198411346,
                },
                CalibrationKnot {
                    input: 0.59375,
                    coverage: 0.5534675845075272,
                },
                CalibrationKnot {
                    input: 0.6015625,
                    coverage: 0.5681283488419184,
                },
                CalibrationKnot {
                    input: 0.609375,
                    coverage: 0.5830328957790822,
                },
                CalibrationKnot {
                    input: 0.625,
                    coverage: 0.6133153278417018,
                },
                CalibrationKnot {
                    input: 0.640625,
                    coverage: 0.6443644243550536,
                },
                CalibrationKnot {
                    input: 0.65625,
                    coverage: 0.6761801583833088,
                },
                CalibrationKnot {
                    input: 0.671875,
                    coverage: 0.708762535417303,
                },
                CalibrationKnot {
                    input: 0.6875,
                    coverage: 0.74211155436211,
                },
                CalibrationKnot {
                    input: 0.703125,
                    coverage: 0.7762272369239368,
                },
                CalibrationKnot {
                    input: 0.70703125,
                    coverage: 0.7848759408120258,
                },
                CalibrationKnot {
                    input: 0.708984375,
                    coverage: 0.7889972013105222,
                },
                CalibrationKnot {
                    input: 0.7109375,
                    coverage: 0.7928774259407053,
                },
                CalibrationKnot {
                    input: 0.71484375,
                    coverage: 0.8002219866259698,
                },
                CalibrationKnot {
                    input: 0.71875,
                    coverage: 0.8071793990760735,
                },
                CalibrationKnot {
                    input: 0.72265625,
                    coverage: 0.8138423566755062,
                },
                CalibrationKnot {
                    input: 0.7265625,
                    coverage: 0.8202418956254651,
                },
                CalibrationKnot {
                    input: 0.734375,
                    coverage: 0.8323957405611172,
                },
                CalibrationKnot {
                    input: 0.7421875,
                    coverage: 0.8437868574232077,
                },
                CalibrationKnot {
                    input: 0.75,
                    coverage: 0.8545335103688028,
                },
                CalibrationKnot {
                    input: 0.7578125,
                    coverage: 0.8646907456361884,
                },
                CalibrationKnot {
                    input: 0.765625,
                    coverage: 0.8743237930405455,
                },
                CalibrationKnot {
                    input: 0.7734375,
                    coverage: 0.8834544202983986,
                },
                CalibrationKnot {
                    input: 0.78125,
                    coverage: 0.8921355832692325,
                },
                CalibrationKnot {
                    input: 0.7890625,
                    coverage: 0.900371172714283,
                },
                CalibrationKnot {
                    input: 0.796875,
                    coverage: 0.9082001772074131,
                },
                CalibrationKnot {
                    input: 0.8046875,
                    coverage: 0.9156379220865769,
                },
                CalibrationKnot {
                    input: 0.8125,
                    coverage: 0.9226866450148393,
                },
                CalibrationKnot {
                    input: 0.8203125,
                    coverage: 0.9293772997217403,
                },
                CalibrationKnot {
                    input: 0.828125,
                    coverage: 0.9357541476706621,
                },
                CalibrationKnot {
                    input: 0.8359375,
                    coverage: 0.9417481242171638,
                },
                CalibrationKnot {
                    input: 0.84375,
                    coverage: 0.9474135671850721,
                },
                CalibrationKnot {
                    input: 0.8515625,
                    coverage: 0.9527504847593715,
                },
                CalibrationKnot {
                    input: 0.859375,
                    coverage: 0.9577694570765394,
                },
                CalibrationKnot {
                    input: 0.8671875,
                    coverage: 0.9624856620623052,
                },
                CalibrationKnot {
                    input: 0.875,
                    coverage: 0.9668992427933614,
                },
                CalibrationKnot {
                    input: 0.8828125,
                    coverage: 0.9710103218214384,
                },
                CalibrationKnot {
                    input: 0.890625,
                    coverage: 0.974831529664594,
                },
                CalibrationKnot {
                    input: 0.8984375,
                    coverage: 0.9783712213198927,
                },
                CalibrationKnot {
                    input: 0.90625,
                    coverage: 0.9816294074647265,
                },
                CalibrationKnot {
                    input: 0.9140625,
                    coverage: 0.9846060666221773,
                },
                CalibrationKnot {
                    input: 0.921875,
                    coverage: 0.9873060386351181,
                },
                CalibrationKnot {
                    input: 0.9296875,
                    coverage: 0.9897411873527685,
                },
                CalibrationKnot {
                    input: 0.9375,
                    coverage: 0.9919120690062582,
                },
                CalibrationKnot {
                    input: 0.9453125,
                    coverage: 0.9938186894286418,
                },
                CalibrationKnot {
                    input: 0.953125,
                    coverage: 0.995461033815413,
                },
                CalibrationKnot {
                    input: 0.9609375,
                    coverage: 0.9968430158209654,
                },
                CalibrationKnot {
                    input: 0.96875,
                    coverage: 0.9979744833807974,
                },
                CalibrationKnot {
                    input: 0.97265625,
                    coverage: 0.9984464482832021,
                },
                CalibrationKnot {
                    input: 0.9765625,
                    coverage: 0.998855904622809,
                },
                CalibrationKnot {
                    input: 0.98046875,
                    coverage: 0.9992028491664589,
                },
                CalibrationKnot {
                    input: 0.984375,
                    coverage: 0.999487286749397,
                },
                CalibrationKnot {
                    input: 0.98828125,
                    coverage: 0.9997092104358748,
                },
                CalibrationKnot {
                    input: 0.990234375,
                    coverage: 0.9997967294070449,
                },
                CalibrationKnot {
                    input: 0.9921875,
                    coverage: 0.9998686240748339,
                },
                CalibrationKnot {
                    input: 0.994140625,
                    coverage: 0.9999248902202534,
                },
                CalibrationKnot {
                    input: 0.99609375,
                    coverage: 0.9999655276662743,
                },
                CalibrationKnot {
                    input: 0.998046875,
                    coverage: 0.9999905384049145,
                },
                CalibrationKnot {
                    input: 1.0,
                    coverage: 0.9999999211977438,
                },
                CalibrationKnot {
                    input: 1.5,
                    coverage: 1.0,
                },
                CalibrationKnot {
                    input: 2.0,
                    coverage: 1.0,
                },
            ],
            None,
            None,
        ),
        _ => return None,
    })
}

/// Returns the ranked layout's genuine additional occupancy observations, separate from mark size.
pub(super) fn dispersion(id: &str) -> Option<&'static [CalibrationKnot]> {
    (id == "ranked-blue-noise-marks").then_some(&[
        CalibrationKnot {
            input: 0.0,
            coverage: 0.0,
        },
        CalibrationKnot {
            input: 0.0001,
            coverage: 0.0,
        },
        CalibrationKnot {
            input: 0.00025,
            coverage: 0.00037384033203125,
        },
        CalibrationKnot {
            input: 0.0005,
            coverage: 0.0007476806640625,
        },
        CalibrationKnot {
            input: 0.001,
            coverage: 0.001495361328125,
        },
        CalibrationKnot {
            input: 0.0025,
            coverage: 0.0037384033203125,
        },
        CalibrationKnot {
            input: 0.005,
            coverage: 0.007476806640625,
        },
        CalibrationKnot {
            input: 0.0075,
            coverage: 0.01158905029296875,
        },
        CalibrationKnot {
            input: 0.01,
            coverage: 0.01532745361328125,
        },
        CalibrationKnot {
            input: 0.015,
            coverage: 0.02280426025390625,
        },
        CalibrationKnot {
            input: 0.02,
            coverage: 0.0306549072265625,
        },
        CalibrationKnot {
            input: 0.025,
            coverage: 0.0394744873046875,
        },
        CalibrationKnot {
            input: 0.05,
            coverage: 0.07783699035644531,
        },
        CalibrationKnot {
            input: 0.075,
            coverage: 0.11592864990234375,
        },
        CalibrationKnot {
            input: 0.1,
            coverage: 0.1507129669189453,
        },
        CalibrationKnot {
            input: 0.125,
            coverage: 0.19281768798828125,
        },
        CalibrationKnot {
            input: 0.15,
            coverage: 0.2347869873046875,
        },
        CalibrationKnot {
            input: 0.175,
            coverage: 0.27483177185058594,
        },
        CalibrationKnot {
            input: 0.2,
            coverage: 0.3114337921142578,
        },
        CalibrationKnot {
            input: 0.225,
            coverage: 0.3450279235839844,
        },
        CalibrationKnot {
            input: 0.25,
            coverage: 0.3808746337890625,
        },
        CalibrationKnot {
            input: 0.275,
            coverage: 0.41034889221191406,
        },
        CalibrationKnot {
            input: 0.3,
            coverage: 0.444976806640625,
        },
        CalibrationKnot {
            input: 0.325,
            coverage: 0.47656822204589844,
        },
        CalibrationKnot {
            input: 0.35,
            coverage: 0.5125007629394531,
        },
        CalibrationKnot {
            input: 0.375,
            coverage: 0.5415191650390625,
        },
        CalibrationKnot {
            input: 0.4,
            coverage: 0.5719203948974609,
        },
        CalibrationKnot {
            input: 0.425,
            coverage: 0.5984439849853516,
        },
        CalibrationKnot {
            input: 0.45,
            coverage: 0.6287956237792969,
        },
        CalibrationKnot {
            input: 0.475,
            coverage: 0.6499805450439453,
        },
        CalibrationKnot {
            input: 0.5,
            coverage: 0.6758060455322266,
        },
        CalibrationKnot {
            input: 0.525,
            coverage: 0.6991653442382812,
        },
        CalibrationKnot {
            input: 0.55,
            coverage: 0.723907470703125,
        },
        CalibrationKnot {
            input: 0.575,
            coverage: 0.7445583343505859,
        },
        CalibrationKnot {
            input: 0.6,
            coverage: 0.7639331817626953,
        },
        CalibrationKnot {
            input: 0.625,
            coverage: 0.7869434356689453,
        },
        CalibrationKnot {
            input: 0.65,
            coverage: 0.8019199371337891,
        },
        CalibrationKnot {
            input: 0.675,
            coverage: 0.8259925842285156,
        },
        CalibrationKnot {
            input: 0.7,
            coverage: 0.8430366516113281,
        },
        CalibrationKnot {
            input: 0.725,
            coverage: 0.8646144866943359,
        },
        CalibrationKnot {
            input: 0.75,
            coverage: 0.8787574768066406,
        },
        CalibrationKnot {
            input: 0.775,
            coverage: 0.8927154541015625,
        },
        CalibrationKnot {
            input: 0.8,
            coverage: 0.9048843383789062,
        },
        CalibrationKnot {
            input: 0.825,
            coverage: 0.9213733673095703,
        },
        CalibrationKnot {
            input: 0.85,
            coverage: 0.9333286285400391,
        },
        CalibrationKnot {
            input: 0.875,
            coverage: 0.9443359375,
        },
        CalibrationKnot {
            input: 0.9,
            coverage: 0.9539031982421875,
        },
        CalibrationKnot {
            input: 0.925,
            coverage: 0.9656448364257812,
        },
        CalibrationKnot {
            input: 0.95,
            coverage: 0.9795608520507812,
        },
        CalibrationKnot {
            input: 0.975,
            coverage: 0.9886932373046875,
        },
        CalibrationKnot {
            input: 0.98,
            coverage: 0.991302490234375,
        },
        CalibrationKnot {
            input: 0.99,
            coverage: 0.9960861206054688,
        },
        CalibrationKnot {
            input: 0.995,
            coverage: 0.9973907470703125,
        },
        CalibrationKnot {
            input: 0.999,
            coverage: 1.0,
        },
        CalibrationKnot {
            input: 1.0,
            coverage: 1.0,
        },
    ])
}
