//! Checked-in typed reference measurements for immutable bundled Pattern recipes.
//! Numeric values are reconstructed from current validated artifacts; runtime never loads JSON.

use toniator_domain::{
    CalibrationBinding, CalibrationKnot, CalibrationVariation, CanvasSpec, ChannelId, Document,
    DocumentCommand, PatternDefinitionRecipe, SavedCalibration, SourceReference,
    UniformCalibration, ValidationError,
};

type VariationData = (f64, f64, f64, f64, Option<f64>, Option<f64>);
type UniformData = (f64, &'static [CalibrationKnot]);

/// Materializes one recipe on the canonical reference document before binding it.
///
/// # Errors
/// Rejects invalid default-document construction or a recipe the domain cannot install.
fn canonical_reference_binding(
    recipe: &PatternDefinitionRecipe,
) -> Result<CalibrationBinding, ValidationError> {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )?;
    let base = document.pattern_settings().clone();
    let base_definition = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.id == base.definition_id)
        .ok_or_else(|| {
            ValidationError::new(
                "calibration.reference.definition",
                "default Pattern definition is missing",
            )
        })?
        .definition
        .clone();
    let (document, _) =
        document.apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe.clone(),
        })?;
    CalibrationBinding::for_channel(&document, ChannelId(1))
}

const CLUSTERED_DISPERSION_RANDOM_LINKS_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 0.000707820030800886,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 0.0035155762580536573,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.00698621928301831,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.020550642995669183,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.03377483585538419,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.06615721852613088,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.09033853652510704,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.11473752809438201,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.13944685468898055,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.16454782226804934,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.18577633866046273,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.20728605734676106,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.22904327097356647,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.25099410712303505,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.33962601610631765,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.3616752435268884,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.3835195186361889,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.4050793124069412,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.42636025680825096,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.4473473072970553,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.4680040854003308,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.4883350975801598,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.5083308307839886,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.5279637299698553,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.547225764080164,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.566108938205123,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.5845850212836448,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.6026421786223835,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.6202450571146327,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.6374031201035932,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 0.6540824307610992,
    },
    CalibrationKnot {
        input: 1.03125,
        coverage: 0.6702584810833822,
    },
    CalibrationKnot {
        input: 1.0625,
        coverage: 0.685892886966229,
    },
    CalibrationKnot {
        input: 1.09375,
        coverage: 0.7009701810768141,
    },
    CalibrationKnot {
        input: 1.125,
        coverage: 0.7155194454989816,
    },
    CalibrationKnot {
        input: 1.15625,
        coverage: 0.7295543595946961,
    },
    CalibrationKnot {
        input: 1.1875,
        coverage: 0.7430645186736549,
    },
    CalibrationKnot {
        input: 1.21875,
        coverage: 0.7560878786799483,
    },
    CalibrationKnot {
        input: 1.25,
        coverage: 0.7686258983121,
    },
    CalibrationKnot {
        input: 1.28125,
        coverage: 0.780692959894776,
    },
    CalibrationKnot {
        input: 1.3125,
        coverage: 0.7922941274680685,
    },
    CalibrationKnot {
        input: 1.34375,
        coverage: 0.8034593035032658,
    },
    CalibrationKnot {
        input: 1.375,
        coverage: 0.8141856731336918,
    },
    CalibrationKnot {
        input: 1.40625,
        coverage: 0.8244641272711641,
    },
    CalibrationKnot {
        input: 1.4375,
        coverage: 0.8342831198164462,
    },
    CalibrationKnot {
        input: 1.46875,
        coverage: 0.8436396685433551,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 0.8525623108814004,
    },
    CalibrationKnot {
        input: 1.53125,
        coverage: 0.8611029524789471,
    },
    CalibrationKnot {
        input: 1.5625,
        coverage: 0.8692914629137589,
    },
    CalibrationKnot {
        input: 1.59375,
        coverage: 0.8771108876347556,
    },
    CalibrationKnot {
        input: 1.625,
        coverage: 0.8845313821920051,
    },
    CalibrationKnot {
        input: 1.65625,
        coverage: 0.8915716142445475,
    },
    CalibrationKnot {
        input: 1.6875,
        coverage: 0.8982729945529535,
    },
    CalibrationKnot {
        input: 1.71875,
        coverage: 0.9046528149614731,
    },
    CalibrationKnot {
        input: 1.75,
        coverage: 0.9107118423118576,
    },
    CalibrationKnot {
        input: 1.78125,
        coverage: 0.9164518477216609,
    },
    CalibrationKnot {
        input: 1.8125,
        coverage: 0.9218966942387223,
    },
    CalibrationKnot {
        input: 1.84375,
        coverage: 0.9270784611534051,
    },
    CalibrationKnot {
        input: 1.875,
        coverage: 0.9320130890307231,
    },
    CalibrationKnot {
        input: 1.90625,
        coverage: 0.9367297944348745,
    },
    CalibrationKnot {
        input: 1.9375,
        coverage: 0.9412088632427723,
    },
    CalibrationKnot {
        input: 1.96875,
        coverage: 0.9454335894719231,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 0.9494036374635424,
    },
];

const CURVE_MOTIF_ROWS_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 0.0013573062433124242,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 0.006784530118415258,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.013564758599878442,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.06764148843974914,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.13483260511568518,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.18492876340431968,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.2347745320128986,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.2843650588689316,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.3337027317738646,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.4153694065476844,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.4963382946353254,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.576591154083435,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.656156299228405,
    },
    CalibrationKnot {
        input: 0.515625,
        coverage: 0.6759326254065788,
    },
    CalibrationKnot {
        input: 0.5234375,
        coverage: 0.6858042339278294,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.6954405533810536,
    },
    CalibrationKnot {
        input: 0.546875,
        coverage: 0.7141269745343518,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.7322905249614924,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.7500246330100848,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.7673383518277382,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.7842729893705784,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.8008468277446102,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.8170743203110677,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.8329668146473249,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.8485335229712406,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.8637827069287523,
    },
    CalibrationKnot {
        input: 0.6953125,
        coverage: 0.8710131307741686,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.8779393851520509,
    },
    CalibrationKnot {
        input: 0.7109375,
        coverage: 0.884641142900132,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.8911237449391691,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.9035128143423001,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.9151948362334417,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.9262061697158661,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.9365787636864196,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.9463371379285057,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.9554913825655319,
    },
    CalibrationKnot {
        input: 0.828125,
        coverage: 0.9640589975412908,
    },
    CalibrationKnot {
        input: 0.8359375,
        coverage: 0.9681176366689177,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.9715915629337435,
    },
    CalibrationKnot {
        input: 0.8515625,
        coverage: 0.9747132969476531,
    },
    CalibrationKnot {
        input: 0.859375,
        coverage: 0.977544467587177,
    },
    CalibrationKnot {
        input: 0.8671875,
        coverage: 0.9800862872438887,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.9823402034549029,
    },
    CalibrationKnot {
        input: 0.890625,
        coverage: 0.9863501097739369,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.9900699265739019,
    },
    CalibrationKnot {
        input: 0.921875,
        coverage: 0.9935072669771632,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.9965714725614051,
    },
    CalibrationKnot {
        input: 0.9453125,
        coverage: 0.9975214589090071,
    },
    CalibrationKnot {
        input: 0.953125,
        coverage: 0.9982777322320062,
    },
    CalibrationKnot {
        input: 0.9609375,
        coverage: 0.9988860719150027,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.9993599356284895,
    },
    CalibrationKnot {
        input: 0.97265625,
        coverage: 0.9995479489537604,
    },
    CalibrationKnot {
        input: 0.9765625,
        coverage: 0.9997033509218991,
    },
    CalibrationKnot {
        input: 0.98046875,
        coverage: 0.9998261409816483,
    },
    CalibrationKnot {
        input: 0.982421875,
        coverage: 0.9998753047337712,
    },
    CalibrationKnot {
        input: 0.984375,
        coverage: 0.9999163177712475,
    },
    CalibrationKnot {
        input: 0.986328125,
        coverage: 0.9999491764210136,
    },
    CalibrationKnot {
        input: 0.98828125,
        coverage: 0.9999738828595246,
    },
    CalibrationKnot {
        input: 0.990234375,
        coverage: 0.9999904358335743,
    },
    CalibrationKnot {
        input: 0.9921875,
        coverage: 0.9999988398469994,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const EVEN_RANDOM_CIRCLES_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 6.06115516363084e-07,
    },
    CalibrationKnot {
        input: 0.003,
        coverage: 7.71267959719887e-06,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 2.142391341650417e-05,
    },
    CalibrationKnot {
        input: 0.0075,
        coverage: 4.967954266896907e-05,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 8.831890778470704e-05,
    },
    CalibrationKnot {
        input: 0.0125,
        coverage: 0.00014354749070923963,
    },
    CalibrationKnot {
        input: 0.015,
        coverage: 0.00020670392853028806,
    },
    CalibrationKnot {
        input: 0.0175,
        coverage: 0.00028139508082133024,
    },
    CalibrationKnot {
        input: 0.02,
        coverage: 0.0003675387597895999,
    },
    CalibrationKnot {
        input: 0.025,
        coverage: 0.0005825636643334222,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.0008389262624031442,
    },
    CalibrationKnot {
        input: 0.04,
        coverage: 0.0014955558888036778,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.0023511559471348226,
    },
    CalibrationKnot {
        input: 0.0625,
        coverage: 0.0036739012393642704,
    },
    CalibrationKnot {
        input: 0.07500000000000001,
        coverage: 0.005309345570373802,
    },
    CalibrationKnot {
        input: 0.08750000000000001,
        coverage: 0.007231844882877215,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.009460518517251031,
    },
    CalibrationKnot {
        input: 0.11875000000000001,
        coverage: 0.013348285206914424,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.017921407469085777,
    },
    CalibrationKnot {
        input: 0.15625,
        coverage: 0.023148466224237954,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.029056545125230773,
    },
    CalibrationKnot {
        input: 0.19374999999999998,
        coverage: 0.03562317444390784,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.042871038374644614,
    },
    CalibrationKnot {
        input: 0.23125,
        coverage: 0.050778265509571416,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.05934834099649086,
    },
    CalibrationKnot {
        input: 0.265625,
        coverage: 0.06703345135838557,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.07515455456549375,
    },
    CalibrationKnot {
        input: 0.296875,
        coverage: 0.08374980996722053,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.09282633239662891,
    },
    CalibrationKnot {
        input: 0.328125,
        coverage: 0.10234448087844535,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.11233687358032554,
    },
    CalibrationKnot {
        input: 0.359375,
        coverage: 0.12278400209014216,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.13373500701436786,
    },
    CalibrationKnot {
        input: 0.390625,
        coverage: 0.1451157822103234,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.15696112187280284,
    },
    CalibrationKnot {
        input: 0.421875,
        coverage: 0.16928309157100163,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.18209066102525856,
    },
    CalibrationKnot {
        input: 0.453125,
        coverage: 0.19534639382361,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.20905754265042537,
    },
    CalibrationKnot {
        input: 0.484375,
        coverage: 0.22323593911621306,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.23787896577111078,
    },
    CalibrationKnot {
        input: 0.515625,
        coverage: 0.25304388042419723,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.26862019073167703,
    },
    CalibrationKnot {
        input: 0.546875,
        coverage: 0.2846620004740674,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.30116997304495674,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.3181446108291154,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.335621859131619,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.35353014400145033,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.371930541920494,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.39077350309960657,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.4100838152733396,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.42989998389521866,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.45014505900759566,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.47085662290781727,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.4920189657343045,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.5348385174645898,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.5774504414193407,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.5984717411757636,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.6192325238225391,
    },
    CalibrationKnot {
        input: 0.828125,
        coverage: 0.6396658605216272,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.6597591441856115,
    },
    CalibrationKnot {
        input: 0.859375,
        coverage: 0.6794016491990781,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.6985823789287207,
    },
    CalibrationKnot {
        input: 0.890625,
        coverage: 0.7172784072820773,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.7354863109390481,
    },
    CalibrationKnot {
        input: 0.921875,
        coverage: 0.7531185682340322,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.7701915800564781,
    },
    CalibrationKnot {
        input: 0.953125,
        coverage: 0.7866825338046306,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.8025809289099175,
    },
    CalibrationKnot {
        input: 0.984375,
        coverage: 0.8178887568261424,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 0.8325348643466417,
    },
    CalibrationKnot {
        input: 1.015625,
        coverage: 0.8465132497347576,
    },
    CalibrationKnot {
        input: 1.03125,
        coverage: 0.8598530267755351,
    },
    CalibrationKnot {
        input: 1.046875,
        coverage: 0.8724753124856617,
    },
    CalibrationKnot {
        input: 1.0625,
        coverage: 0.8844189451974931,
    },
    CalibrationKnot {
        input: 1.078125,
        coverage: 0.8956651943153952,
    },
    CalibrationKnot {
        input: 1.09375,
        coverage: 0.9062357740563649,
    },
    CalibrationKnot {
        input: 1.109375,
        coverage: 0.9161299261860774,
    },
    CalibrationKnot {
        input: 1.125,
        coverage: 0.9253691133002508,
    },
    CalibrationKnot {
        input: 1.140625,
        coverage: 0.9339540394140955,
    },
    CalibrationKnot {
        input: 1.15625,
        coverage: 0.9418803471474124,
    },
    CalibrationKnot {
        input: 1.171875,
        coverage: 0.9491652973325546,
    },
    CalibrationKnot {
        input: 1.1875,
        coverage: 0.9558273238973902,
    },
    CalibrationKnot {
        input: 1.203125,
        coverage: 0.9618995693927062,
    },
    CalibrationKnot {
        input: 1.21875,
        coverage: 0.967400659949624,
    },
    CalibrationKnot {
        input: 1.234375,
        coverage: 0.9723517414320186,
    },
    CalibrationKnot {
        input: 1.25,
        coverage: 0.9767947120796965,
    },
    CalibrationKnot {
        input: 1.265625,
        coverage: 0.9807309218245899,
    },
    CalibrationKnot {
        input: 1.28125,
        coverage: 0.9841911528641785,
    },
    CalibrationKnot {
        input: 1.296875,
        coverage: 0.9871917825355849,
    },
    CalibrationKnot {
        input: 1.3125,
        coverage: 0.9897694917918372,
    },
    CalibrationKnot {
        input: 1.328125,
        coverage: 0.9919522743291685,
    },
    CalibrationKnot {
        input: 1.34375,
        coverage: 0.9937722965824998,
    },
    CalibrationKnot {
        input: 1.359375,
        coverage: 0.9952622300535794,
    },
    CalibrationKnot {
        input: 1.375,
        coverage: 0.9964672480388311,
    },
    CalibrationKnot {
        input: 1.390625,
        coverage: 0.9974189196678142,
    },
    CalibrationKnot {
        input: 1.40625,
        coverage: 0.9981443898143925,
    },
    CalibrationKnot {
        input: 1.421875,
        coverage: 0.9986874372406855,
    },
    CalibrationKnot {
        input: 1.4375,
        coverage: 0.9990896888303202,
    },
    CalibrationKnot {
        input: 1.453125,
        coverage: 0.9993817723470649,
    },
    CalibrationKnot {
        input: 1.46875,
        coverage: 0.9995877660106604,
    },
    CalibrationKnot {
        input: 1.484375,
        coverage: 0.9997311096792426,
    },
    CalibrationKnot {
        input: 1.4921875,
        coverage: 0.999784764135603,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 0.9998275385391939,
    },
    CalibrationKnot {
        input: 1.5078125,
        coverage: 0.9998619683486036,
    },
    CalibrationKnot {
        input: 1.515625,
        coverage: 0.9998898556136058,
    },
    CalibrationKnot {
        input: 1.5234375,
        coverage: 0.9999123550103768,
    },
    CalibrationKnot {
        input: 1.53125,
        coverage: 0.9999306770535088,
    },
    CalibrationKnot {
        input: 1.546875,
        coverage: 0.9999577615690076,
    },
    CalibrationKnot {
        input: 1.5625,
        coverage: 0.9999755653119715,
    },
    CalibrationKnot {
        input: 1.578125,
        coverage: 0.9999866192840736,
    },
    CalibrationKnot {
        input: 1.59375,
        coverage: 0.9999931070864786,
    },
    CalibrationKnot {
        input: 1.625,
        coverage: 0.99999890124799,
    },
    CalibrationKnot {
        input: 1.75,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const GRID_VORONOI_SCALE_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 1.000000003261102e-06,
    },
    CalibrationKnot {
        input: 0.003,
        coverage: 8.999999994377197e-06,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 2.5000000000110373e-05,
    },
    CalibrationKnot {
        input: 0.0075,
        coverage: 5.625069000057672e-05,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.00010000000000039198,
    },
    CalibrationKnot {
        input: 0.0125,
        coverage: 0.00015625017499686853,
    },
    CalibrationKnot {
        input: 0.015,
        coverage: 0.00022499999998900158,
    },
    CalibrationKnot {
        input: 0.0175,
        coverage: 0.0003062496850014303,
    },
    CalibrationKnot {
        input: 0.02,
        coverage: 0.00039999999999813785,
    },
    CalibrationKnot {
        input: 0.025,
        coverage: 0.0006250000000044652,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.0008999999999847188,
    },
    CalibrationKnot {
        input: 0.035,
        coverage: 0.0012250000000001953,
    },
    CalibrationKnot {
        input: 0.04,
        coverage: 0.0016000000000032208,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.00250000000000568,
    },
    CalibrationKnot {
        input: 0.0625,
        coverage: 0.003906254000002459,
    },
    CalibrationKnot {
        input: 0.07500000000000001,
        coverage: 0.005624999999996492,
    },
    CalibrationKnot {
        input: 0.08750000000000001,
        coverage: 0.007656246675000996,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.010000000000006068,
    },
    CalibrationKnot {
        input: 0.11875000000000001,
        coverage: 0.014101566300004824,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.018906251924999687,
    },
    CalibrationKnot {
        input: 0.15625,
        coverage: 0.024414057500006258,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.030624999999997165,
    },
    CalibrationKnot {
        input: 0.19374999999999998,
        coverage: 0.037539068700003966,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.04515624192499822,
    },
    CalibrationKnot {
        input: 0.23125,
        coverage: 0.053476555100005284,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.062499999999999986,
    },
    CalibrationKnot {
        input: 0.265625,
        coverage: 0.07055665337500372,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.07910155350001066,
    },
    CalibrationKnot {
        input: 0.296875,
        coverage: 0.08813477037498782,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.0976562699999903,
    },
    CalibrationKnot {
        input: 0.328125,
        coverage: 0.10766601037501639,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.1181640735000207,
    },
    CalibrationKnot {
        input: 0.359375,
        coverage: 0.1291503733749976,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.14062499999999997,
    },
    CalibrationKnot {
        input: 0.390625,
        coverage: 0.15258790937500435,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.16503904949999257,
    },
    CalibrationKnot {
        input: 0.421875,
        coverage: 0.1779785223750066,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.19140627799999968,
    },
    CalibrationKnot {
        input: 0.453125,
        coverage: 0.20532225837499635,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.21972657749999888,
    },
    CalibrationKnot {
        input: 0.484375,
        coverage: 0.23461911737499772,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.24999999999999997,
    },
    CalibrationKnot {
        input: 0.515625,
        coverage: 0.26586916537499905,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.2822265454999972,
    },
    CalibrationKnot {
        input: 0.546875,
        coverage: 0.2990722743749971,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.316406286000003,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.33422850637500545,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.3525390814999956,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.37133786137500174,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.390625,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.41040042137499844,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.4306640415000184,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.4514160263750168,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.47265629400001075,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.4943847543749883,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.5166015855000126,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.5393066053750026,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.5625,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.5861816773749915,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.6103515374999977,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.635009778375,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.660156301999996,
    },
    CalibrationKnot {
        input: 0.828125,
        coverage: 0.6857910023750055,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.7119140895000089,
    },
    CalibrationKnot {
        input: 0.859375,
        coverage: 0.7385253493750004,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.765625,
    },
    CalibrationKnot {
        input: 0.890625,
        coverage: 0.7932129333750102,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.8212890335000029,
    },
    CalibrationKnot {
        input: 0.921875,
        coverage: 0.8498535303749879,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.8789063100000075,
    },
    CalibrationKnot {
        input: 0.953125,
        coverage: 0.9084472503750171,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.9384765934999877,
    },
    CalibrationKnot {
        input: 0.984375,
        coverage: 0.9689940933750081,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const ONE_GUIDE_LINES_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 0.0010000000000000009,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 0.0050000000000014495,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.009999999999999898,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.04999999999999827,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.10000000000000092,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.24999999999999994,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.49999999999999994,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.75,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const ROUND_SPIRAL_LINE_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 0.001001429852400392,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 0.00500716727120518,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.010014392114853266,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.05008500597056024,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.10015335756423366,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.2502749913036191,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.5005774653070333,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.750748972821327,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.8756702436522669,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.9381002167107677,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.9692858506096841,
    },
    CalibrationKnot {
        input: 0.984375,
        coverage: 0.9848472181202077,
    },
    CalibrationKnot {
        input: 0.9921875,
        coverage: 0.992596222796615,
    },
    CalibrationKnot {
        input: 0.99609375,
        coverage: 0.9964475718447336,
    },
    CalibrationKnot {
        input: 0.998046875,
        coverage: 0.9983421083267896,
    },
    CalibrationKnot {
        input: 0.9990234375,
        coverage: 0.9992212561045555,
    },
    CalibrationKnot {
        input: 0.99951171875,
        coverage: 0.9995951284111758,
    },
    CalibrationKnot {
        input: 0.999755859375,
        coverage: 0.9997427460315291,
    },
    CalibrationKnot {
        input: 0.9998779296875,
        coverage: 0.9997997363336006,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 0.9998449311978617,
    },
    CalibrationKnot {
        input: 1.0001220703125,
        coverage: 0.9998786415634222,
    },
    CalibrationKnot {
        input: 1.000244140625,
        coverage: 0.9999025813177189,
    },
    CalibrationKnot {
        input: 1.00048828125,
        coverage: 0.9999383921321328,
    },
    CalibrationKnot {
        input: 1.000732421875,
        coverage: 0.9999631108700405,
    },
    CalibrationKnot {
        input: 1.0009765625,
        coverage: 0.9999797126164589,
    },
    CalibrationKnot {
        input: 1.001220703125,
        coverage: 0.9999901336900741,
    },
    CalibrationKnot {
        input: 1.00146484375,
        coverage: 0.9999960258388465,
    },
    CalibrationKnot {
        input: 1.001953125,
        coverage: 0.9999998087885822,
    },
    CalibrationKnot {
        input: 1.00390625,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.0078125,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.015625,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.03125,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.0625,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.125,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.25,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const ROUND_SPIRAL_MARKS_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 1.412481753624507e-06,
    },
    CalibrationKnot {
        input: 0.003,
        coverage: 1.3504720972684133e-05,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 3.752135888197956e-05,
    },
    CalibrationKnot {
        input: 0.00625,
        coverage: 5.978932787398895e-05,
    },
    CalibrationKnot {
        input: 0.0075,
        coverage: 8.61027612190764e-05,
    },
    CalibrationKnot {
        input: 0.00875,
        coverage: 0.00011725145593629583,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.0001531640483220981,
    },
    CalibrationKnot {
        input: 0.0125,
        coverage: 0.00024154685711989658,
    },
    CalibrationKnot {
        input: 0.015,
        coverage: 0.00034784665601601345,
    },
    CalibrationKnot {
        input: 0.02,
        coverage: 0.0006214328553555368,
    },
    CalibrationKnot {
        input: 0.025,
        coverage: 0.000973738259448224,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.0014019518237377404,
    },
    CalibrationKnot {
        input: 0.04,
        coverage: 0.002496662943890824,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.0039051512952753475,
    },
    CalibrationKnot {
        input: 0.0625,
        coverage: 0.006105912478679242,
    },
    CalibrationKnot {
        input: 0.07500000000000001,
        coverage: 0.008797461823313561,
    },
    CalibrationKnot {
        input: 0.08750000000000001,
        coverage: 0.01198038214761136,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.015656369089677255,
    },
    CalibrationKnot {
        input: 0.109375,
        coverage: 0.018738054870515847,
    },
    CalibrationKnot {
        input: 0.11875000000000001,
        coverage: 0.02209055280173152,
    },
    CalibrationKnot {
        input: 0.12812500000000002,
        coverage: 0.025726609288534485,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.029633047450810402,
    },
    CalibrationKnot {
        input: 0.146875,
        coverage: 0.03382394999988716,
    },
    CalibrationKnot {
        input: 0.15625,
        coverage: 0.0382848813232449,
    },
    CalibrationKnot {
        input: 0.165625,
        coverage: 0.04303014093732406,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.04804461404299483,
    },
    CalibrationKnot {
        input: 0.18437499999999998,
        coverage: 0.05333461768530464,
    },
    CalibrationKnot {
        input: 0.19374999999999998,
        coverage: 0.058909730553911924,
    },
    CalibrationKnot {
        input: 0.203125,
        coverage: 0.06475309005918521,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.07088265364402305,
    },
    CalibrationKnot {
        input: 0.221875,
        coverage: 0.07727900213754492,
    },
    CalibrationKnot {
        input: 0.23125,
        coverage: 0.0839597086999791,
    },
    CalibrationKnot {
        input: 0.240625,
        coverage: 0.09090843214808464,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.09813345440046081,
    },
    CalibrationKnot {
        input: 0.265625,
        coverage: 0.11080769589655441,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.12423217165249582,
    },
    CalibrationKnot {
        input: 0.296875,
        coverage: 0.13842776900978418,
    },
    CalibrationKnot {
        input: 0.3046875,
        coverage: 0.14581515742051473,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.153415981999555,
    },
    CalibrationKnot {
        input: 0.328125,
        coverage: 0.16915865205176414,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.18566916344981968,
    },
    CalibrationKnot {
        input: 0.3515625,
        coverage: 0.19421079887489606,
    },
    CalibrationKnot {
        input: 0.359375,
        coverage: 0.20296423285061443,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.22100411489990684,
    },
    CalibrationKnot {
        input: 0.3828125,
        coverage: 0.2303098625011305,
    },
    CalibrationKnot {
        input: 0.390625,
        coverage: 0.23982868998363016,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.2594047755692923,
    },
    CalibrationKnot {
        input: 0.421875,
        coverage: 0.27975530878386157,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.3008786493630686,
    },
    CalibrationKnot {
        input: 0.453125,
        coverage: 0.32277313220210274,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.34543917214525927,
    },
    CalibrationKnot {
        input: 0.484375,
        coverage: 0.3688714783710682,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.39309277252969277,
    },
    CalibrationKnot {
        input: 0.515625,
        coverage: 0.4180569144204679,
    },
    CalibrationKnot {
        input: 0.5234375,
        coverage: 0.43082468725710127,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.44380934045363224,
    },
    CalibrationKnot {
        input: 0.546875,
        coverage: 0.4703024325544896,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.4975585704906133,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.5255785441305139,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.5543650756717573,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.5839173420882084,
    },
    CalibrationKnot {
        input: 0.6171875,
        coverage: 0.5989822002298363,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.6142637561761902,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.6453542155591001,
    },
    CalibrationKnot {
        input: 0.6484375,
        coverage: 0.6611846271105257,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.677230523378048,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.7098421646363852,
    },
    CalibrationKnot {
        input: 0.6796875,
        coverage: 0.7264276199996756,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.7432262168689834,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.7772848642938589,
    },
    CalibrationKnot {
        input: 0.70703125,
        coverage: 0.7858602259781409,
    },
    CalibrationKnot {
        input: 0.7109375,
        coverage: 0.794029569556477,
    },
    CalibrationKnot {
        input: 0.71484375,
        coverage: 0.8018748143466154,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.8094504216902065,
    },
    CalibrationKnot {
        input: 0.7265625,
        coverage: 0.8239687808449507,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.8377363606829926,
    },
    CalibrationKnot {
        input: 0.7421875,
        coverage: 0.8508249876953015,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.8632785691546527,
    },
    CalibrationKnot {
        input: 0.7578125,
        coverage: 0.8751219964701883,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.8863734510366146,
    },
    CalibrationKnot {
        input: 0.7734375,
        coverage: 0.8970498750765845,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.9071547322659532,
    },
    CalibrationKnot {
        input: 0.7890625,
        coverage: 0.9167023540616048,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.925704691470553,
    },
    CalibrationKnot {
        input: 0.8046875,
        coverage: 0.9341561188220517,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.9420508667373201,
    },
    CalibrationKnot {
        input: 0.8203125,
        coverage: 0.9493997146839835,
    },
    CalibrationKnot {
        input: 0.828125,
        coverage: 0.9562083128696506,
    },
    CalibrationKnot {
        input: 0.8359375,
        coverage: 0.96246974689768,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.968167143065785,
    },
    CalibrationKnot {
        input: 0.8515625,
        coverage: 0.9733044036909944,
    },
    CalibrationKnot {
        input: 0.859375,
        coverage: 0.9778738369772474,
    },
    CalibrationKnot {
        input: 0.8671875,
        coverage: 0.981886459691675,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.985350614028295,
    },
    CalibrationKnot {
        input: 0.8828125,
        coverage: 0.9882869637883265,
    },
    CalibrationKnot {
        input: 0.890625,
        coverage: 0.9907507257399805,
    },
    CalibrationKnot {
        input: 0.8984375,
        coverage: 0.9927713814942599,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.9944246260833591,
    },
    CalibrationKnot {
        input: 0.9140625,
        coverage: 0.9957750792503098,
    },
    CalibrationKnot {
        input: 0.921875,
        coverage: 0.9968830989472817,
    },
    CalibrationKnot {
        input: 0.9296875,
        coverage: 0.9977750665805578,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.9984621395886215,
    },
    CalibrationKnot {
        input: 0.9453125,
        coverage: 0.9989635871839762,
    },
    CalibrationKnot {
        input: 0.953125,
        coverage: 0.999320250172546,
    },
    CalibrationKnot {
        input: 0.9609375,
        coverage: 0.9995888923779841,
    },
    CalibrationKnot {
        input: 0.96484375,
        coverage: 0.9996935618937013,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.9997788856869627,
    },
    CalibrationKnot {
        input: 0.97265625,
        coverage: 0.9998503375577305,
    },
    CalibrationKnot {
        input: 0.9765625,
        coverage: 0.9999044215261126,
    },
    CalibrationKnot {
        input: 0.98046875,
        coverage: 0.9999419947674332,
    },
    CalibrationKnot {
        input: 0.984375,
        coverage: 0.9999680932665103,
    },
    CalibrationKnot {
        input: 0.98828125,
        coverage: 0.9999843936256398,
    },
    CalibrationKnot {
        input: 0.9921875,
        coverage: 0.9999924890723713,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 0.9999970845596722,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const SOURCE_WEIGHTED_DISPERSION_VORONOI_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 9.952981034698077e-07,
    },
    CalibrationKnot {
        input: 0.003,
        coverage: 8.957688659466903e-06,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 2.4882470081659643e-05,
    },
    CalibrationKnot {
        input: 0.0075,
        coverage: 5.598557143132453e-05,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 9.952989964700245e-05,
    },
    CalibrationKnot {
        input: 0.0125,
        coverage: 0.00015551545718586437,
    },
    CalibrationKnot {
        input: 0.015,
        coverage: 0.00022394226566531622,
    },
    CalibrationKnot {
        input: 0.0175,
        coverage: 0.0003048129778268693,
    },
    CalibrationKnot {
        input: 0.02,
        coverage: 0.0003981346451814398,
    },
    CalibrationKnot {
        input: 0.025,
        coverage: 0.0006221262024897735,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.000895874459241279,
    },
    CalibrationKnot {
        input: 0.035,
        coverage: 0.001219333005709364,
    },
    CalibrationKnot {
        input: 0.04,
        coverage: 0.001592446840215995,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.002487971315083711,
    },
    CalibrationKnot {
        input: 0.0625,
        coverage: 0.0038873926315741254,
    },
    CalibrationKnot {
        input: 0.07500000000000001,
        coverage: 0.005598442491030184,
    },
    CalibrationKnot {
        input: 0.08750000000000001,
        coverage: 0.007621282669420667,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.009955771690055737,
    },
    CalibrationKnot {
        input: 0.11875000000000001,
        coverage: 0.01404191955252485,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.018826702337011413,
    },
    CalibrationKnot {
        input: 0.15625,
        coverage: 0.024308298937390223,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.03049170580235355,
    },
    CalibrationKnot {
        input: 0.19374999999999998,
        coverage: 0.03738028838220248,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.044975323606305934,
    },
    CalibrationKnot {
        input: 0.23125,
        coverage: 0.05327657191246116,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.062279907964228984,
    },
    CalibrationKnot {
        input: 0.265625,
        coverage: 0.07031747858739269,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.07884066605710627,
    },
    CalibrationKnot {
        input: 0.296875,
        coverage: 0.08785114566561147,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.09735167938073121,
    },
    CalibrationKnot {
        input: 0.328125,
        coverage: 0.10734169257147198,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.11782082869177717,
    },
    CalibrationKnot {
        input: 0.359375,
        coverage: 0.12878825506224784,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.14024300872216805,
    },
    CalibrationKnot {
        input: 0.390625,
        coverage: 0.15218665469527157,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.16461809638557592,
    },
    CalibrationKnot {
        input: 0.421875,
        coverage: 0.177537869897254,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.19094472259493384,
    },
    CalibrationKnot {
        input: 0.453125,
        coverage: 0.2048372091817831,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.21921721989979784,
    },
    CalibrationKnot {
        input: 0.484375,
        coverage: 0.2340880777277631,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.24945005959413896,
    },
    CalibrationKnot {
        input: 0.515625,
        coverage: 0.26530345271312844,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.2816473278736867,
    },
    CalibrationKnot {
        input: 0.546875,
        coverage: 0.29848123782819436,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.3158045714237541,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.33361779313386253,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.3519242282694231,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.37072617680007486,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.39002054623136206,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.4098047850776512,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.4300794318043395,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.4508455190251846,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.4721015145969186,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.493847096789554,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.5160832139934162,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.5388101949790411,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.5620269920001527,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.5857315520443367,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.6099267765655286,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.634611008841421,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.6597832828646923,
    },
    CalibrationKnot {
        input: 0.828125,
        coverage: 0.6854434823532976,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.7115919772550166,
    },
    CalibrationKnot {
        input: 0.859375,
        coverage: 0.7382289387975349,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.7653549720482693,
    },
    CalibrationKnot {
        input: 0.890625,
        coverage: 0.7929727192093377,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.8210816889040713,
    },
    CalibrationKnot {
        input: 0.921875,
        coverage: 0.849681010357432,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.878770084085329,
    },
    CalibrationKnot {
        input: 0.953125,
        coverage: 0.9083478335225983,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.9384126034416892,
    },
    CalibrationKnot {
        input: 0.984375,
        coverage: 0.9689622632242739,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 0.9999999891999677,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const SQUARE_SPIRAL_MARKS_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 1.0019134980228709e-06,
    },
    CalibrationKnot {
        input: 0.003,
        coverage: 1.275028039013045e-05,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 3.54174539027679e-05,
    },
    CalibrationKnot {
        input: 0.0075,
        coverage: 8.460306127386744e-05,
    },
    CalibrationKnot {
        input: 0.00875,
        coverage: 0.00011515441732977595,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.00015040572267356503,
    },
    CalibrationKnot {
        input: 0.01125,
        coverage: 0.000190358063233802,
    },
    CalibrationKnot {
        input: 0.0125,
        coverage: 0.0002396509032890584,
    },
    CalibrationKnot {
        input: 0.015,
        coverage: 0.0003450967652161584,
    },
    CalibrationKnot {
        input: 0.0175,
        coverage: 0.0004698773262171175,
    },
    CalibrationKnot {
        input: 0.02,
        coverage: 0.0006137177738542148,
    },
    CalibrationKnot {
        input: 0.025,
        coverage: 0.0009676649375897077,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.001393437501380687,
    },
    CalibrationKnot {
        input: 0.04,
        coverage: 0.0024897023313048155,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.0039019840483180525,
    },
    CalibrationKnot {
        input: 0.0625,
        coverage: 0.00609686612990125,
    },
    CalibrationKnot {
        input: 0.07500000000000001,
        coverage: 0.008796908542234527,
    },
    CalibrationKnot {
        input: 0.08750000000000001,
        coverage: 0.011989801631552957,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.01566015206140496,
    },
    CalibrationKnot {
        input: 0.109375,
        coverage: 0.018751777547545794,
    },
    CalibrationKnot {
        input: 0.11875000000000001,
        coverage: 0.022104135329520033,
    },
    CalibrationKnot {
        input: 0.12812500000000002,
        coverage: 0.025732514254081813,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.02965728373665283,
    },
    CalibrationKnot {
        input: 0.146875,
        coverage: 0.033839332936726786,
    },
    CalibrationKnot {
        input: 0.15625,
        coverage: 0.03831752995771575,
    },
    CalibrationKnot {
        input: 0.165625,
        coverage: 0.04305357351537805,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.048065507723188426,
    },
    CalibrationKnot {
        input: 0.18437499999999998,
        coverage: 0.05335332549439562,
    },
    CalibrationKnot {
        input: 0.19374999999999998,
        coverage: 0.058941773820317934,
    },
    CalibrationKnot {
        input: 0.203125,
        coverage: 0.06478381346030115,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.07090186837978185,
    },
    CalibrationKnot {
        input: 0.221875,
        coverage: 0.07732156249884874,
    },
    CalibrationKnot {
        input: 0.23125,
        coverage: 0.08399382780465871,
    },
    CalibrationKnot {
        input: 0.240625,
        coverage: 0.09094308003804422,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.09816761318523638,
    },
    CalibrationKnot {
        input: 0.265625,
        coverage: 0.11085189068631339,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.12427685588502033,
    },
    CalibrationKnot {
        input: 0.2890625,
        coverage: 0.13127702098112853,
    },
    CalibrationKnot {
        input: 0.296875,
        coverage: 0.1384993946733494,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.15346193786954185,
    },
    CalibrationKnot {
        input: 0.328125,
        coverage: 0.16919178741279614,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.185722785323656,
    },
    CalibrationKnot {
        input: 0.359375,
        coverage: 0.20299039711968048,
    },
    CalibrationKnot {
        input: 0.3671875,
        coverage: 0.21191199742993985,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.22105995898708702,
    },
    CalibrationKnot {
        input: 0.390625,
        coverage: 0.2398654293357544,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.25943843905475994,
    },
    CalibrationKnot {
        input: 0.4140625,
        coverage: 0.26951324166164364,
    },
    CalibrationKnot {
        input: 0.421875,
        coverage: 0.27981567621850556,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.300926594724417,
    },
    CalibrationKnot {
        input: 0.4453125,
        coverage: 0.31176993987067864,
    },
    CalibrationKnot {
        input: 0.453125,
        coverage: 0.32284166308646856,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.34549050200762665,
    },
    CalibrationKnot {
        input: 0.484375,
        coverage: 0.3689070686670583,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.39309142303496597,
    },
    CalibrationKnot {
        input: 0.5078125,
        coverage: 0.40547150159921985,
    },
    CalibrationKnot {
        input: 0.515625,
        coverage: 0.4181177373948873,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.44384215917696174,
    },
    CalibrationKnot {
        input: 0.546875,
        coverage: 0.4703344668280305,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.4975946692258516,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.5256227715505515,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.5544187591482088,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.5839826421081074,
    },
    CalibrationKnot {
        input: 0.6171875,
        coverage: 0.5990525313126285,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.6143983928655143,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.6455014959886708,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.6773733785369088,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.710013248402115,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.7433617760791613,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.7773093290286675,
    },
    CalibrationKnot {
        input: 0.70703125,
        coverage: 0.7858996571336361,
    },
    CalibrationKnot {
        input: 0.708984375,
        coverage: 0.7900096816463652,
    },
    CalibrationKnot {
        input: 0.7109375,
        coverage: 0.7938694219424094,
    },
    CalibrationKnot {
        input: 0.71484375,
        coverage: 0.8012389701478105,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.8081646709087636,
    },
    CalibrationKnot {
        input: 0.72265625,
        coverage: 0.8147987419964102,
    },
    CalibrationKnot {
        input: 0.7265625,
        coverage: 0.8211720404773011,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.8332782813356391,
    },
    CalibrationKnot {
        input: 0.7421875,
        coverage: 0.8446265876719282,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.8553340104435677,
    },
    CalibrationKnot {
        input: 0.7578125,
        coverage: 0.8654542005264938,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.8750513423998242,
    },
    CalibrationKnot {
        input: 0.7734375,
        coverage: 0.8842326006566454,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.8928831470676234,
    },
    CalibrationKnot {
        input: 0.7890625,
        coverage: 0.9010896778015436,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.9088898684527656,
    },
    CalibrationKnot {
        input: 0.8046875,
        coverage: 0.9162991763583165,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.9233199966012402,
    },
    CalibrationKnot {
        input: 0.8203125,
        coverage: 0.9299825143263069,
    },
    CalibrationKnot {
        input: 0.828125,
        coverage: 0.9362969988120972,
    },
    CalibrationKnot {
        input: 0.8359375,
        coverage: 0.9422634607438181,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.9478943237923442,
    },
    CalibrationKnot {
        input: 0.8515625,
        coverage: 0.9532101996107323,
    },
    CalibrationKnot {
        input: 0.859375,
        coverage: 0.958211450951422,
    },
    CalibrationKnot {
        input: 0.8671875,
        coverage: 0.9628980851019715,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.9672782849001856,
    },
    CalibrationKnot {
        input: 0.8828125,
        coverage: 0.9713693391118413,
    },
    CalibrationKnot {
        input: 0.890625,
        coverage: 0.975171829048421,
    },
    CalibrationKnot {
        input: 0.8984375,
        coverage: 0.9786857283821566,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.9819110925879616,
    },
    CalibrationKnot {
        input: 0.9140625,
        coverage: 0.984858960543927,
    },
    CalibrationKnot {
        input: 0.921875,
        coverage: 0.987539002162512,
    },
    CalibrationKnot {
        input: 0.9296875,
        coverage: 0.9899513036094711,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.9920958687643585,
    },
    CalibrationKnot {
        input: 0.9453125,
        coverage: 0.9939726849223898,
    },
    CalibrationKnot {
        input: 0.953125,
        coverage: 0.9955837645844714,
    },
    CalibrationKnot {
        input: 0.9609375,
        coverage: 0.9969413869534343,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.9980477891533007,
    },
    CalibrationKnot {
        input: 0.97265625,
        coverage: 0.9985069556317392,
    },
    CalibrationKnot {
        input: 0.9765625,
        coverage: 0.9989033948291257,
    },
    CalibrationKnot {
        input: 0.98046875,
        coverage: 0.9992370897274655,
    },
    CalibrationKnot {
        input: 0.984375,
        coverage: 0.9995093106609878,
    },
    CalibrationKnot {
        input: 0.98828125,
        coverage: 0.9997217041494765,
    },
    CalibrationKnot {
        input: 0.990234375,
        coverage: 0.9998054640570181,
    },
    CalibrationKnot {
        input: 0.9921875,
        coverage: 0.999874267967501,
    },
    CalibrationKnot {
        input: 0.994140625,
        coverage: 0.999928116356623,
    },
    CalibrationKnot {
        input: 0.99609375,
        coverage: 0.9999670084689374,
    },
    CalibrationKnot {
        input: 0.998046875,
        coverage: 0.9999909449783162,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 0.9999999246269577,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const STRAIGHT_GRID_CIRCLES_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 9.999921361903101e-07,
    },
    CalibrationKnot {
        input: 0.003,
        coverage: 1.2725428246461671e-05,
    },
    CalibrationKnot {
        input: 0.004,
        coverage: 2.262298356064702e-05,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 3.7528253397233654e-05,
    },
    CalibrationKnot {
        input: 0.00625,
        coverage: 5.863805769756203e-05,
    },
    CalibrationKnot {
        input: 0.0075,
        coverage: 8.443861618975029e-05,
    },
    CalibrationKnot {
        input: 0.00875,
        coverage: 0.00011493016353409741,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.00015307682389572013,
    },
    CalibrationKnot {
        input: 0.0125,
        coverage: 0.00023926558789489494,
    },
    CalibrationKnot {
        input: 0.015,
        coverage: 0.0003445423475020449,
    },
    CalibrationKnot {
        input: 0.02,
        coverage: 0.0006180979061878309,
    },
    CalibrationKnot {
        input: 0.025,
        coverage: 0.0009657787214419331,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.0013977260325307263,
    },
    CalibrationKnot {
        input: 0.035,
        coverage: 0.0019024606833674658,
    },
    CalibrationKnot {
        input: 0.04,
        coverage: 0.002492405822616179,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.003902111350290669,
    },
    CalibrationKnot {
        input: 0.0625,
        coverage: 0.006097050245416741,
    },
    CalibrationKnot {
        input: 0.07500000000000001,
        coverage: 0.00879165726620968,
    },
    CalibrationKnot {
        input: 0.08750000000000001,
        coverage: 0.011977731449394792,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.01564467652791625,
    },
    CalibrationKnot {
        input: 0.109375,
        coverage: 0.018728990903067334,
    },
    CalibrationKnot {
        input: 0.11875000000000001,
        coverage: 0.022077272750877102,
    },
    CalibrationKnot {
        input: 0.12812500000000002,
        coverage: 0.025714456129414375,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.029615219662132486,
    },
    CalibrationKnot {
        input: 0.146875,
        coverage: 0.03380552787702553,
    },
    CalibrationKnot {
        input: 0.15625,
        coverage: 0.03825885787491511,
    },
    CalibrationKnot {
        input: 0.165625,
        coverage: 0.04298765676399149,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.0480078457753484,
    },
    CalibrationKnot {
        input: 0.18437499999999998,
        coverage: 0.053289841749865674,
    },
    CalibrationKnot {
        input: 0.19374999999999998,
        coverage: 0.05884692624751601,
    },
    CalibrationKnot {
        input: 0.203125,
        coverage: 0.06469700858363006,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.07080684818537486,
    },
    CalibrationKnot {
        input: 0.221875,
        coverage: 0.07719232835706098,
    },
    CalibrationKnot {
        input: 0.23125,
        coverage: 0.08387186304322553,
    },
    CalibrationKnot {
        input: 0.240625,
        coverage: 0.09081012903182821,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.09802408628903617,
    },
    CalibrationKnot {
        input: 0.2578125,
        coverage: 0.104246322533674,
    },
    CalibrationKnot {
        input: 0.265625,
        coverage: 0.11068017950044846,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.12408435597311133,
    },
    CalibrationKnot {
        input: 0.2890625,
        coverage: 0.13107367142090828,
    },
    CalibrationKnot {
        input: 0.296875,
        coverage: 0.13827606487230582,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.1532144731989709,
    },
    CalibrationKnot {
        input: 0.3203125,
        coverage: 0.16097122768603458,
    },
    CalibrationKnot {
        input: 0.328125,
        coverage: 0.1689410888185913,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.18541379677473155,
    },
    CalibrationKnot {
        input: 0.3515625,
        coverage: 0.19393747489267882,
    },
    CalibrationKnot {
        input: 0.359375,
        coverage: 0.2026755939542097,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.2206826884000942,
    },
    CalibrationKnot {
        input: 0.390625,
        coverage: 0.23945604543208687,
    },
    CalibrationKnot {
        input: 0.3984375,
        coverage: 0.24913006497409107,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.2590416580267628,
    },
    CalibrationKnot {
        input: 0.421875,
        coverage: 0.27935111814221814,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.3004269945180697,
    },
    CalibrationKnot {
        input: 0.453125,
        coverage: 0.3222692774764142,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.3448779364735846,
    },
    CalibrationKnot {
        input: 0.484375,
        coverage: 0.3682529885460754,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.3924480833479156,
    },
    CalibrationKnot {
        input: 0.515625,
        coverage: 0.41735882702903776,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.44303653749682476,
    },
    CalibrationKnot {
        input: 0.546875,
        coverage: 0.4694807670435178,
    },
    CalibrationKnot {
        input: 0.5546875,
        coverage: 0.4829903331156526,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.49674100140461436,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.524721019841104,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.5534675845075006,
    },
    CalibrationKnot {
        input: 0.6015625,
        coverage: 0.5681283488419201,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.5830328957790752,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.6133153278416886,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.64436442435507,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.6761801583833102,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.7087625354173215,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.7421115543620799,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.7762272369239388,
    },
    CalibrationKnot {
        input: 0.70703125,
        coverage: 0.7848759408120309,
    },
    CalibrationKnot {
        input: 0.708984375,
        coverage: 0.7889972013085517,
    },
    CalibrationKnot {
        input: 0.7109375,
        coverage: 0.7928774259406919,
    },
    CalibrationKnot {
        input: 0.71484375,
        coverage: 0.8002219866240613,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.8071793990760662,
    },
    CalibrationKnot {
        input: 0.72265625,
        coverage: 0.8138423566755252,
    },
    CalibrationKnot {
        input: 0.7265625,
        coverage: 0.8202418956183625,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.8323957405610606,
    },
    CalibrationKnot {
        input: 0.7421875,
        coverage: 0.8437868574175139,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.8545335103683002,
    },
    CalibrationKnot {
        input: 0.7578125,
        coverage: 0.8646907456299123,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.8743237930393599,
    },
    CalibrationKnot {
        input: 0.7734375,
        coverage: 0.883454420289788,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.8921355832655679,
    },
    CalibrationKnot {
        input: 0.7890625,
        coverage: 0.9003711727013752,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.9082001771993193,
    },
    CalibrationKnot {
        input: 0.8046875,
        coverage: 0.9156379220833116,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.9226866450148541,
    },
    CalibrationKnot {
        input: 0.8203125,
        coverage: 0.9293772997217531,
    },
    CalibrationKnot {
        input: 0.828125,
        coverage: 0.9357541476564495,
    },
    CalibrationKnot {
        input: 0.8359375,
        coverage: 0.9417481242171588,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.9474135671850858,
    },
    CalibrationKnot {
        input: 0.8515625,
        coverage: 0.9527504847589204,
    },
    CalibrationKnot {
        input: 0.859375,
        coverage: 0.9577694570765433,
    },
    CalibrationKnot {
        input: 0.8671875,
        coverage: 0.9624856620622954,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.9668992427933689,
    },
    CalibrationKnot {
        input: 0.8828125,
        coverage: 0.9710103218033045,
    },
    CalibrationKnot {
        input: 0.890625,
        coverage: 0.9748315296645892,
    },
    CalibrationKnot {
        input: 0.8984375,
        coverage: 0.9783712213102912,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.9816294074647501,
    },
    CalibrationKnot {
        input: 0.9140625,
        coverage: 0.9846060666221811,
    },
    CalibrationKnot {
        input: 0.921875,
        coverage: 0.987306038617705,
    },
    CalibrationKnot {
        input: 0.9296875,
        coverage: 0.989741187352792,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.9919120690062703,
    },
    CalibrationKnot {
        input: 0.9453125,
        coverage: 0.9938186894235009,
    },
    CalibrationKnot {
        input: 0.953125,
        coverage: 0.9954610338143255,
    },
    CalibrationKnot {
        input: 0.9609375,
        coverage: 0.9968430158013462,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.9979744833807902,
    },
    CalibrationKnot {
        input: 0.97265625,
        coverage: 0.998446448269425,
    },
    CalibrationKnot {
        input: 0.9765625,
        coverage: 0.9988559046228138,
    },
    CalibrationKnot {
        input: 0.98046875,
        coverage: 0.9992028491664521,
    },
    CalibrationKnot {
        input: 0.984375,
        coverage: 0.9994872867493991,
    },
    CalibrationKnot {
        input: 0.98828125,
        coverage: 0.9997092104358678,
    },
    CalibrationKnot {
        input: 0.990234375,
        coverage: 0.9997967294020771,
    },
    CalibrationKnot {
        input: 0.9921875,
        coverage: 0.9998686240748141,
    },
    CalibrationKnot {
        input: 0.994140625,
        coverage: 0.9999248902172437,
    },
    CalibrationKnot {
        input: 0.99609375,
        coverage: 0.9999655276662894,
    },
    CalibrationKnot {
        input: 0.998046875,
        coverage: 0.9999905384038361,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 0.9999999211976324,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const THREE_GUIDE_CELLS_SCALE_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 9.923100001766992e-07,
    },
    CalibrationKnot {
        input: 0.003,
        coverage: 8.930955000290461e-06,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 2.4808025000091824e-05,
    },
    CalibrationKnot {
        input: 0.0075,
        coverage: 5.5818032947686056e-05,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 9.923210000079598e-05,
    },
    CalibrationKnot {
        input: 0.0125,
        coverage: 0.0001550497870161591,
    },
    CalibrationKnot {
        input: 0.015,
        coverage: 0.00022327222499941114,
    },
    CalibrationKnot {
        input: 0.0175,
        coverage: 0.0003038977852178079,
    },
    CalibrationKnot {
        input: 0.02,
        coverage: 0.00039692840000084024,
    },
    CalibrationKnot {
        input: 0.025,
        coverage: 0.0006202006250000389,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.0008930888999999096,
    },
    CalibrationKnot {
        input: 0.035,
        coverage: 0.0012155932250000247,
    },
    CalibrationKnot {
        input: 0.04,
        coverage: 0.0015877136000002794,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.002480802499998118,
    },
    CalibrationKnot {
        input: 0.0625,
        coverage: 0.003876254166504137,
    },
    CalibrationKnot {
        input: 0.07500000000000001,
        coverage: 0.005581805625000036,
    },
    CalibrationKnot {
        input: 0.08750000000000001,
        coverage: 0.007597454530115594,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.00992321000000011,
    },
    CalibrationKnot {
        input: 0.11875000000000001,
        coverage: 0.013993276854159874,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.018761065013362437,
    },
    CalibrationKnot {
        input: 0.15625,
        coverage: 0.02422657798794722,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.03038982100000089,
    },
    CalibrationKnot {
        input: 0.19374999999999998,
        coverage: 0.03725078979494446,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.044809487472195125,
    },
    CalibrationKnot {
        input: 0.23125,
        coverage: 0.05306590276591269,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.06202004875000057,
    },
    CalibrationKnot {
        input: 0.265625,
        coverage: 0.07001482241979398,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.07849412553455601,
    },
    CalibrationKnot {
        input: 0.296875,
        coverage: 0.0874596561030184,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.09691789210029117,
    },
    CalibrationKnot {
        input: 0.328125,
        coverage: 0.1068694670404309,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.11731438599907874,
    },
    CalibrationKnot {
        input: 0.359375,
        coverage: 0.12825262362407538,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.1396842245698594,
    },
    CalibrationKnot {
        input: 0.390625,
        coverage: 0.15160918865290934,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.1640274476618345,
    },
    CalibrationKnot {
        input: 0.421875,
        coverage: 0.17693907296851055,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.19034406381981472,
    },
    CalibrationKnot {
        input: 0.453125,
        coverage: 0.20424234445736716,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.21863399412493303,
    },
    CalibrationKnot {
        input: 0.484375,
        coverage: 0.23351898135809812,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.24889731401066886,
    },
    CalibrationKnot {
        input: 0.515625,
        coverage: 0.2647689880391278,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.2811339992812193,
    },
    CalibrationKnot {
        input: 0.546875,
        coverage: 0.2979923561718133,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.31534405450563724,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.3331890899621166,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.3515274712135962,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.37035915525431196,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.38968421849293383,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.4095026590131566,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.4298143644377954,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.4506194520373122,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.4719179193596118,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.4937096463790447,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.5159941222364917,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.538764817835355,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.5620200506825884,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.585759815503134,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.6099841061112672,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.634692934045265,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.6598862942652457,
    },
    CalibrationKnot {
        input: 0.828125,
        coverage: 0.6855641337363209,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.7117266034193457,
    },
    CalibrationKnot {
        input: 0.859375,
        coverage: 0.7383735045304092,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.7655049908955182,
    },
    CalibrationKnot {
        input: 0.890625,
        coverage: 0.7931210585376873,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.8212215520798742,
    },
    CalibrationKnot {
        input: 0.921875,
        coverage: 0.8498066337989529,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.8788762989097935,
    },
    CalibrationKnot {
        input: 0.953125,
        coverage: 0.9084303853735658,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.9384690624466347,
    },
    CalibrationKnot {
        input: 0.984375,
        coverage: 0.9689922629904042,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const THREE_GUIDE_MAZE_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 0.0017337146583317803,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 0.008665910283223648,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.017326397795389255,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.0864083239078676,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.17226594078320473,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.23629982882281075,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.30002070441083784,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.3634239971295156,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.42653158366406974,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.4788800786582195,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.5309459299791031,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.5827339537950258,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.6342628606356684,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.685513520883781,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.7365089411035526,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.7872214925896177,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.837669834592971,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.8878457448496847,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.9377946476558174,
    },
    CalibrationKnot {
        input: 0.5703125,
        coverage: 0.9502323722489487,
    },
    CalibrationKnot {
        input: 0.57421875,
        coverage: 0.9564467893420852,
    },
    CalibrationKnot {
        input: 0.576171875,
        coverage: 0.9595475243217875,
    },
    CalibrationKnot {
        input: 0.5771484375,
        coverage: 0.9610968542172349,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.9620952268201348,
    },
    CalibrationKnot {
        input: 0.58203125,
        coverage: 0.9650371381291017,
    },
    CalibrationKnot {
        input: 0.5859375,
        coverage: 0.9676037720461135,
    },
    CalibrationKnot {
        input: 0.58984375,
        coverage: 0.9699032813831648,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.9719873679053194,
    },
    CalibrationKnot {
        input: 0.6015625,
        coverage: 0.9755880468916747,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.9785476935558088,
    },
    CalibrationKnot {
        input: 0.6171875,
        coverage: 0.9809417468604673,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.9829346568987984,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.9865392639814351,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.9897028570862902,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.9924475992867154,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.9946796638605919,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.9964767717162244,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.9978748783974648,
    },
    CalibrationKnot {
        input: 0.7265625,
        coverage: 0.9984311481259748,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.9988960379966948,
    },
    CalibrationKnot {
        input: 0.7421875,
        coverage: 0.9992700490393636,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.9995554614693313,
    },
    CalibrationKnot {
        input: 0.75390625,
        coverage: 0.9996654822640296,
    },
    CalibrationKnot {
        input: 0.7578125,
        coverage: 0.9997539811675085,
    },
    CalibrationKnot {
        input: 0.76171875,
        coverage: 0.9998212800919313,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.9998670346682259,
    },
    CalibrationKnot {
        input: 0.767578125,
        coverage: 0.9998818800898285,
    },
    CalibrationKnot {
        input: 0.76953125,
        coverage: 0.9998914079631279,
    },
    CalibrationKnot {
        input: 0.7734375,
        coverage: 0.9999031980965748,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.9999241520655537,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.9999561500907628,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.9999748850661181,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.9999893675254162,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.9999977000878019,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const TRIAGRID_CUSTOM_SHAPE_MARKS_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 1.2890345052657182e-06,
    },
    CalibrationKnot {
        input: 0.003,
        coverage: 1.1601596335942095e-05,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 3.2226338947062474e-05,
    },
    CalibrationKnot {
        input: 0.00625,
        coverage: 5.035424999695036e-05,
    },
    CalibrationKnot {
        input: 0.0075,
        coverage: 7.250997709913407e-05,
    },
    CalibrationKnot {
        input: 0.00875,
        coverage: 9.869399657031139e-05,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.00012890630841255036,
    },
    CalibrationKnot {
        input: 0.0125,
        coverage: 0.00020141580919880958,
    },
    CalibrationKnot {
        input: 0.015,
        coverage: 0.0002900384794582272,
    },
    CalibrationKnot {
        input: 0.0175,
        coverage: 0.0003947759862885298,
    },
    CalibrationKnot {
        input: 0.02,
        coverage: 0.0005156252336509929,
    },
    CalibrationKnot {
        input: 0.025,
        coverage: 0.0008056632367940972,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.001160156775713529,
    },
    CalibrationKnot {
        input: 0.035,
        coverage: 0.0015791006109569239,
    },
    CalibrationKnot {
        input: 0.04,
        coverage: 0.0020625009346022503,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.0032226577103164813,
    },
    CalibrationKnot {
        input: 0.0625,
        coverage: 0.005035401183887406,
    },
    CalibrationKnot {
        input: 0.07500000000000001,
        coverage: 0.0072509762758566545,
    },
    CalibrationKnot {
        input: 0.08750000000000001,
        coverage: 0.00986938298622149,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.012890621314984682,
    },
    CalibrationKnot {
        input: 0.109375,
        coverage: 0.015420916125654759,
    },
    CalibrationKnot {
        input: 0.11875000000000001,
        coverage: 0.018177793748850403,
    },
    CalibrationKnot {
        input: 0.12812500000000002,
        coverage: 0.021161275916395876,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.024371339110288392,
    },
    CalibrationKnot {
        input: 0.146875,
        coverage: 0.027807994642983523,
    },
    CalibrationKnot {
        input: 0.15625,
        coverage: 0.03147125739929531,
    },
    CalibrationKnot {
        input: 0.165625,
        coverage: 0.03536109850268837,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.039477531944885895,
    },
    CalibrationKnot {
        input: 0.18437499999999998,
        coverage: 0.04382057528996433,
    },
    CalibrationKnot {
        input: 0.19374999999999998,
        coverage: 0.048390317418015985,
    },
    CalibrationKnot {
        input: 0.203125,
        coverage: 0.05318961526197796,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.05821960684431294,
    },
    CalibrationKnot {
        input: 0.221875,
        coverage: 0.0634803106261578,
    },
    CalibrationKnot {
        input: 0.23125,
        coverage: 0.06897174870595528,
    },
    CalibrationKnot {
        input: 0.240625,
        coverage: 0.07469387779614467,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.08064671908584523,
    },
    CalibrationKnot {
        input: 0.265625,
        coverage: 0.09108083196712606,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.10215581408999302,
    },
    CalibrationKnot {
        input: 0.296875,
        coverage: 0.11387166545444513,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.12622838606048467,
    },
    CalibrationKnot {
        input: 0.328125,
        coverage: 0.13922597590810668,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.15286443499731617,
    },
    CalibrationKnot {
        input: 0.359375,
        coverage: 0.1671437633281112,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.18206396090049187,
    },
    CalibrationKnot {
        input: 0.390625,
        coverage: 0.197625027714458,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.2138269637700102,
    },
    CalibrationKnot {
        input: 0.421875,
        coverage: 0.23066976906714592,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.24815344360587155,
    },
    CalibrationKnot {
        input: 0.453125,
        coverage: 0.2662779873861811,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.2850434004080755,
    },
    CalibrationKnot {
        input: 0.484375,
        coverage: 0.3044477133847296,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.3244824291699648,
    },
    CalibrationKnot {
        input: 0.515625,
        coverage: 0.34514657010319066,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.3664401361843965,
    },
    CalibrationKnot {
        input: 0.546875,
        coverage: 0.3883631274135961,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.4109155437907763,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.43409738531594455,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.45790865198909286,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.48234934381023364,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.507419460779363,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.5331190028964716,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.5594479701615628,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.6134346903094019,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.7216878103094005,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.9381941783094015,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.9923207703094021,
    },
    CalibrationKnot {
        input: 0.91015625,
        coverage: 0.9990865943094004,
    },
    CalibrationKnot {
        input: 0.91064453125,
        coverage: 0.9999323543094,
    },
    CalibrationKnot {
        input: 0.910675048828125,
        coverage: 0.999985154309402,
    },
    CalibrationKnot {
        input: 0.9106826782226562,
        coverage: 0.9999984023094001,
    },
    CalibrationKnot {
        input: 0.9106903076171875,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.91070556640625,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.9107666015625,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.910888671875,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.9111328125,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.912109375,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.9140625,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.921875,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const TRIAGRID_SPANNING_TREE_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 0.001733415086373732,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 0.00866184655572147,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.017311157314382673,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.051784530260838436,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.08605764600780987,
    },
    CalibrationKnot {
        input: 0.07500000000000001,
        coverage: 0.12862091799760456,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.17087463548243872,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.23366030043370983,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.29571367827881373,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.3570428625454345,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.4176686311410724,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.4676528154570409,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.5171602899334402,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.5662156932036416,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.6148060223075605,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.6629428224102758,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.7106173268241514,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.7578378620031194,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.8045936450087856,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.850822517138843,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.8965548648726684,
    },
    CalibrationKnot {
        input: 0.5703125,
        coverage: 0.9079147614382916,
    },
    CalibrationKnot {
        input: 0.57421875,
        coverage: 0.9135819047338551,
    },
    CalibrationKnot {
        input: 0.576171875,
        coverage: 0.9164099007751657,
    },
    CalibrationKnot {
        input: 0.5771484375,
        coverage: 0.9178212545438347,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.9189604625823238,
    },
    CalibrationKnot {
        input: 0.58203125,
        coverage: 0.922889420745,
    },
    CalibrationKnot {
        input: 0.5859375,
        coverage: 0.9265480282975258,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.9333170662708419,
    },
    CalibrationKnot {
        input: 0.6015625,
        coverage: 0.9395183304766387,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.9452539158906474,
    },
    CalibrationKnot {
        input: 0.6171875,
        coverage: 0.9505825392402174,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.9555855352801359,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.9649457665983703,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.9735453034998898,
    },
    CalibrationKnot {
        input: 0.6640625,
        coverage: 0.9775808301998047,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.981266869770903,
    },
    CalibrationKnot {
        input: 0.6796875,
        coverage: 0.9844047380973598,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.9871706402754009,
    },
    CalibrationKnot {
        input: 0.6953125,
        coverage: 0.9896143266076609,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.9917664853786862,
    },
    CalibrationKnot {
        input: 0.7109375,
        coverage: 0.9936446885666983,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.9952607960449071,
    },
    CalibrationKnot {
        input: 0.7265625,
        coverage: 0.9966266072846734,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.9977528254064215,
    },
    CalibrationKnot {
        input: 0.7421875,
        coverage: 0.9986433467429541,
    },
    CalibrationKnot {
        input: 0.74609375,
        coverage: 0.999002765662096,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.9993058932952343,
    },
    CalibrationKnot {
        input: 0.75390625,
        coverage: 0.999553682051143,
    },
    CalibrationKnot {
        input: 0.7578125,
        coverage: 0.9997463700363697,
    },
    CalibrationKnot {
        input: 0.759765625,
        coverage: 0.9998221375862695,
    },
    CalibrationKnot {
        input: 0.76171875,
        coverage: 0.9998845181420684,
    },
    CalibrationKnot {
        input: 0.763671875,
        coverage: 0.9999334123107065,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.9999689354786743,
    },
    CalibrationKnot {
        input: 0.767578125,
        coverage: 0.9999910902505229,
    },
    CalibrationKnot {
        input: 0.76953125,
        coverage: 0.999999837695007,
    },
    CalibrationKnot {
        input: 0.7734375,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const TWO_GUIDE_CELLS_UNIFORM_OFFSET_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 1.0000000005880685e-06,
    },
    CalibrationKnot {
        input: 0.003,
        coverage: 8.999999994123599e-06,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 2.499999999828819e-05,
    },
    CalibrationKnot {
        input: 0.0075,
        coverage: 5.6250480002521575e-05,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.00010000000000397104,
    },
    CalibrationKnot {
        input: 0.0125,
        coverage: 0.00015625079999968426,
    },
    CalibrationKnot {
        input: 0.015,
        coverage: 0.00022499999998953872,
    },
    CalibrationKnot {
        input: 0.0175,
        coverage: 0.0003062502799980947,
    },
    CalibrationKnot {
        input: 0.02,
        coverage: 0.0003999999999996529,
    },
    CalibrationKnot {
        input: 0.025,
        coverage: 0.0006250000000026303,
    },
    CalibrationKnot {
        input: 0.030000000000000002,
        coverage: 0.000899999999984978,
    },
    CalibrationKnot {
        input: 0.035,
        coverage: 0.001225000000003104,
    },
    CalibrationKnot {
        input: 0.04,
        coverage: 0.001600000000001734,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.002500000000004242,
    },
    CalibrationKnot {
        input: 0.0625,
        coverage: 0.0039062540000013015,
    },
    CalibrationKnot {
        input: 0.07500000000000001,
        coverage: 0.005624999999998257,
    },
    CalibrationKnot {
        input: 0.08750000000000001,
        coverage: 0.0076562499999999105,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.01000000000000137,
    },
    CalibrationKnot {
        input: 0.11875000000000001,
        coverage: 0.01410156630000345,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.018906254399999753,
    },
    CalibrationKnot {
        input: 0.15625,
        coverage: 0.024414057500007684,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.030624999999998674,
    },
    CalibrationKnot {
        input: 0.19374999999999998,
        coverage: 0.03753906870000115,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.045156243199999994,
    },
    CalibrationKnot {
        input: 0.23125,
        coverage: 0.053476555100004694,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.0625,
    },
    CalibrationKnot {
        input: 0.265625,
        coverage: 0.07055665337500264,
    },
    CalibrationKnot {
        input: 0.28125,
        coverage: 0.07910155350001168,
    },
    CalibrationKnot {
        input: 0.296875,
        coverage: 0.08813477037498837,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.09765626999999202,
    },
    CalibrationKnot {
        input: 0.328125,
        coverage: 0.10766601037501598,
    },
    CalibrationKnot {
        input: 0.34375,
        coverage: 0.11816407350002066,
    },
    CalibrationKnot {
        input: 0.359375,
        coverage: 0.12915037337499452,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.140625,
    },
    CalibrationKnot {
        input: 0.390625,
        coverage: 0.1525879093750035,
    },
    CalibrationKnot {
        input: 0.40625,
        coverage: 0.16503904949999273,
    },
    CalibrationKnot {
        input: 0.421875,
        coverage: 0.17797852237500675,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.19140627800000057,
    },
    CalibrationKnot {
        input: 0.453125,
        coverage: 0.20532225837499607,
    },
    CalibrationKnot {
        input: 0.46875,
        coverage: 0.2197265774999991,
    },
    CalibrationKnot {
        input: 0.484375,
        coverage: 0.23461911737499994,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.25,
    },
    CalibrationKnot {
        input: 0.515625,
        coverage: 0.2658691653749997,
    },
    CalibrationKnot {
        input: 0.53125,
        coverage: 0.2822265454999967,
    },
    CalibrationKnot {
        input: 0.546875,
        coverage: 0.29907227437499745,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.31640628600000464,
    },
    CalibrationKnot {
        input: 0.578125,
        coverage: 0.334228506375005,
    },
    CalibrationKnot {
        input: 0.59375,
        coverage: 0.35253908149999563,
    },
    CalibrationKnot {
        input: 0.609375,
        coverage: 0.3713378613749991,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.390625,
    },
    CalibrationKnot {
        input: 0.640625,
        coverage: 0.4104004213749992,
    },
    CalibrationKnot {
        input: 0.65625,
        coverage: 0.43066404150001825,
    },
    CalibrationKnot {
        input: 0.671875,
        coverage: 0.4514160263750158,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.47265629400001247,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.49438475437498974,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.5166015855000111,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.5393066053750067,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.5625,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.5861816773749922,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.6103515375000012,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.6350097783749972,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.6601563019999975,
    },
    CalibrationKnot {
        input: 0.828125,
        coverage: 0.6857910023750091,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.7119140895000037,
    },
    CalibrationKnot {
        input: 0.859375,
        coverage: 0.7385253493750037,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.765625,
    },
    CalibrationKnot {
        input: 0.890625,
        coverage: 0.7932129333750129,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.8212890335000093,
    },
    CalibrationKnot {
        input: 0.921875,
        coverage: 0.8498535303749853,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.8789063100000089,
    },
    CalibrationKnot {
        input: 0.953125,
        coverage: 0.9084472503750205,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.9384765934999807,
    },
    CalibrationKnot {
        input: 0.984375,
        coverage: 0.9689940933750041,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

const TWO_GUIDE_MAZE_RAW: &[CalibrationKnot] = &[
    CalibrationKnot {
        input: 0.0,
        coverage: 0.0,
    },
    CalibrationKnot {
        input: 0.001,
        coverage: 0.00141403539393544,
    },
    CalibrationKnot {
        input: 0.005,
        coverage: 0.007068888647645354,
    },
    CalibrationKnot {
        input: 0.01,
        coverage: 0.01413405012593757,
    },
    CalibrationKnot {
        input: 0.05,
        coverage: 0.07052129018598113,
    },
    CalibrationKnot {
        input: 0.1,
        coverage: 0.14067016569949925,
    },
    CalibrationKnot {
        input: 0.1375,
        coverage: 0.1930404722191495,
    },
    CalibrationKnot {
        input: 0.175,
        coverage: 0.24520336095519785,
    },
    CalibrationKnot {
        input: 0.2125,
        coverage: 0.29715606246059323,
    },
    CalibrationKnot {
        input: 0.25,
        coverage: 0.3489006225346991,
    },
    CalibrationKnot {
        input: 0.3125,
        coverage: 0.434680731363226,
    },
    CalibrationKnot {
        input: 0.375,
        coverage: 0.5198835290581224,
    },
    CalibrationKnot {
        input: 0.4375,
        coverage: 0.6045087116820237,
    },
    CalibrationKnot {
        input: 0.5,
        coverage: 0.6885563775103452,
    },
    CalibrationKnot {
        input: 0.5625,
        coverage: 0.7720256897883679,
    },
    CalibrationKnot {
        input: 0.625,
        coverage: 0.8549183386429919,
    },
    CalibrationKnot {
        input: 0.6875,
        coverage: 0.9372240143542635,
    },
    CalibrationKnot {
        input: 0.703125,
        coverage: 0.9577099187733396,
    },
    CalibrationKnot {
        input: 0.70703125,
        coverage: 0.9628257446416437,
    },
    CalibrationKnot {
        input: 0.7109375,
        coverage: 0.9656629006312392,
    },
    CalibrationKnot {
        input: 0.71875,
        coverage: 0.9704750975506287,
    },
    CalibrationKnot {
        input: 0.7265625,
        coverage: 0.9746660707969999,
    },
    CalibrationKnot {
        input: 0.734375,
        coverage: 0.9783820194796129,
    },
    CalibrationKnot {
        input: 0.7421875,
        coverage: 0.9816958492862073,
    },
    CalibrationKnot {
        input: 0.75,
        coverage: 0.9846527022364437,
    },
    CalibrationKnot {
        input: 0.7578125,
        coverage: 0.9872834702929443,
    },
    CalibrationKnot {
        input: 0.765625,
        coverage: 0.9896136739393466,
    },
    CalibrationKnot {
        input: 0.7734375,
        coverage: 0.9916587908323323,
    },
    CalibrationKnot {
        input: 0.78125,
        coverage: 0.9934347863696047,
    },
    CalibrationKnot {
        input: 0.7890625,
        coverage: 0.9949547777765588,
    },
    CalibrationKnot {
        input: 0.796875,
        coverage: 0.9962267037359074,
    },
    CalibrationKnot {
        input: 0.8046875,
        coverage: 0.9972625462120992,
    },
    CalibrationKnot {
        input: 0.8125,
        coverage: 0.9980662483372282,
    },
    CalibrationKnot {
        input: 0.8203125,
        coverage: 0.9986495541006695,
    },
    CalibrationKnot {
        input: 0.82421875,
        coverage: 0.9988582366677796,
    },
    CalibrationKnot {
        input: 0.828125,
        coverage: 0.9990125293587562,
    },
    CalibrationKnot {
        input: 0.8359375,
        coverage: 0.9992425470351767,
    },
    CalibrationKnot {
        input: 0.84375,
        coverage: 0.9994376330830135,
    },
    CalibrationKnot {
        input: 0.8515625,
        coverage: 0.9995983051806425,
    },
    CalibrationKnot {
        input: 0.859375,
        coverage: 0.9997257935283947,
    },
    CalibrationKnot {
        input: 0.8671875,
        coverage: 0.9998205956155086,
    },
    CalibrationKnot {
        input: 0.87109375,
        coverage: 0.9998560319243934,
    },
    CalibrationKnot {
        input: 0.875,
        coverage: 0.9998835832214831,
    },
    CalibrationKnot {
        input: 0.87890625,
        coverage: 0.9999032492332673,
    },
    CalibrationKnot {
        input: 0.8828125,
        coverage: 0.9999150344217203,
    },
    CalibrationKnot {
        input: 0.890625,
        coverage: 0.9999265001989825,
    },
    CalibrationKnot {
        input: 0.90625,
        coverage: 0.9999463844790177,
    },
    CalibrationKnot {
        input: 0.9375,
        coverage: 0.9999764443966185,
    },
    CalibrationKnot {
        input: 0.96875,
        coverage: 0.9999941372354235,
    },
    CalibrationKnot {
        input: 1.0,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 1.5,
        coverage: 1.0,
    },
    CalibrationKnot {
        input: 2.0,
        coverage: 1.0,
    },
];

/// Builds one immutable typed companion for the exact canonical built-in recipe.
///
/// # Errors
/// Rejects an invalid reference binding or checked-in numeric payload.
pub(super) fn calibration_for_recipe(
    id: &str,
    recipe: &PatternDefinitionRecipe,
) -> Result<Option<SavedCalibration>, ValidationError> {
    let (profile, raw, variation, uniform): (
        &str,
        &[CalibrationKnot],
        Option<VariationData>,
        Option<UniformData>,
    ) = match id {
        "clustered-dispersion-random-links" => (
            "ton018-reference-fixed-canvas1024-spacings32-64-mean-seed-neutral-v5",
            CLUSTERED_DISPERSION_RANDOM_LINKS_RAW,
            Some((
                0.013294697992545679,
                0.014003209454794408,
                0.005869507903346394,
                0.013243000841733021,
                None,
                None,
            )),
            None,
        ),
        "curve-motif-rows" => (
            "ton018-reference-fixed-canvas1024-spacings32-v4",
            CURVE_MOTIF_ROWS_RAW,
            None,
            None,
        ),
        "even-random-circles" => (
            "ton018-reference-fixed-canvas1024-spacings32-64-mean-seed-neutral-v5",
            EVEN_RANDOM_CIRCLES_RAW,
            Some((
                0.0016342725878613074,
                0.0016342725878613074,
                0.0017999871586610316,
                0.06361250298474434,
                None,
                None,
            )),
            None,
        ),
        "grid-voronoi-scale" => (
            "ton018-reference-fixed-canvas1024-spacings32-v4",
            GRID_VORONOI_SCALE_RAW,
            None,
            None,
        ),
        "one-guide-lines" => (
            "ton018-reference-fixed-canvas1024-spacings32-v4",
            ONE_GUIDE_LINES_RAW,
            None,
            None,
        ),
        "round-spiral-line" => (
            "ton018-reference-fixed-canvas1024-spacings32-v4",
            ROUND_SPIRAL_LINE_RAW,
            None,
            None,
        ),
        "round-spiral-marks" => (
            "ton018-reference-fixed-canvas1024-spacings32-v4",
            ROUND_SPIRAL_MARKS_RAW,
            None,
            None,
        ),
        "source-weighted-dispersion-voronoi" => (
            "ton018-reference-fixed-canvas1024-spacings32-seed-neutral-v5",
            SOURCE_WEIGHTED_DISPERSION_VORONOI_RAW,
            None,
            None,
        ),
        "square-spiral-marks" => (
            "ton018-reference-fixed-canvas1024-spacings64-v4",
            SQUARE_SPIRAL_MARKS_RAW,
            None,
            None,
        ),
        "straight-grid-circles" => (
            "ton018-reference-fixed-canvas1024-spacings32-v4",
            STRAIGHT_GRID_CIRCLES_RAW,
            None,
            None,
        ),
        "three-guide-cells-scale" => (
            "ton018-reference-fixed-canvas1024-spacings32-guide-faces-v5",
            THREE_GUIDE_CELLS_SCALE_RAW,
            None,
            None,
        ),
        "three-guide-maze" => (
            "ton018-reference-fixed-canvas1024-spacings64-seed-neutral-v5",
            THREE_GUIDE_MAZE_RAW,
            None,
            None,
        ),
        "triagrid-custom-shape-marks" => (
            "ton018-reference-fixed-canvas1024-spacings32-v4",
            TRIAGRID_CUSTOM_SHAPE_MARKS_RAW,
            None,
            None,
        ),
        "triagrid-spanning-tree" => (
            "ton018-reference-fixed-canvas1024-spacings32-seed-neutral-v5",
            TRIAGRID_SPANNING_TREE_RAW,
            None,
            None,
        ),
        "two-guide-cells-uniform-offset" => (
            "ton018-reference-fixed-canvas1024-spacings32-guide-faces-v5",
            TWO_GUIDE_CELLS_UNIFORM_OFFSET_RAW,
            None,
            None,
        ),
        "two-guide-maze" => (
            "ton018-reference-fixed-canvas1024-spacings32-seed-neutral-v5",
            TWO_GUIDE_MAZE_RAW,
            None,
            None,
        ),
        _ => return Ok(None),
    };
    let binding = canonical_reference_binding(recipe)?;
    let raw = raw.to_vec();
    let uniform = uniform
        .map(|(radius, inverse)| UniformCalibration::new(radius, inverse.to_vec()))
        .transpose()?;
    let calibration = if let Some((
        raw_absolute,
        raw_fraction,
        tone_absolute,
        low_tone_relative,
        uniform_tone_absolute,
        uniform_low_tone_relative,
    )) = variation
    {
        let variation = CalibrationVariation::new(
            raw_absolute,
            raw_fraction,
            tone_absolute,
            low_tone_relative,
            uniform_tone_absolute,
            uniform_low_tone_relative,
        )?;
        SavedCalibration::with_mean_profile(binding, raw, uniform, variation)?
    } else {
        SavedCalibration::with_profile(binding.profile_for_reference(profile), binding, raw, uniform)?
    };
    Ok(Some(calibration))
}
