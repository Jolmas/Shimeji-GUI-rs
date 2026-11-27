//! Tabla de traducción de XML japonés de Shimeji original a Shimeji-EE.
//!
//! Portada de `/home/luis/bin/shimeji-jap-to-wlshm.py`. El orden de las
//! sustituciones es significativo y se conserva exactamente.

pub const REPLACEMENTS: &[(&str, &str)] = &[
    ("<マスコット ", "<Mascot "),
    ("<動作リスト>", "<ActionList>"),
    ("<動作 ", "<Action "),
    ("名前=", "Name="),
    ("\"振り向く\"", "\"Look\""),
    ("種類=", "Type="),
    ("\"組み込み\"", "\"Embedded\""),
    ("クラス=", "Class="),
    ("\"変位\"", "\"Offset\""),
    ("<!-- 立つ系 -->", "<!-- Standing -->"),
    ("\"立つ\"", "\"Stand\""),
    ("\"静止\"", "\"Stay\""),
    ("枠=", "BorderType="),
    ("\"地面\"", "\"Floor\""),
    ("<アニメーション>", "<Animation>"),
    ("<ポーズ ", "<Pose "),
    ("画像=", "Image="),
    ("基準座標=", "ImageAnchor="),
    ("移動速度=", "Velocity="),
    ("長さ=", "Duration="),
    ("</アニメーション>", "</Animation>"),
    ("</動作>", "</Action>"),
    ("\"歩く\"", "\"Walk\""),
    ("\"移動\"", "\"Move\""),
    ("\"走る\"", "\"Run\""),
    ("\"猛ダッシュ\"", "\"Dash\""),
    ("<!-- 座る系 -->", "<!-- Sitting -->"),
    ("\"座る\"", "\"Sit\""),
    ("\"座って見上げる\"", "\"SitAndLookUp\""),
    ("\"座ってマウスを見上げる\"", "\"SitAndLookAtMouse\""),
    ("<アニメーション ", "<Animation "),
    ("条件=", "Condition="),
    ("</アニメーション>", "</Animation>"),
    ("\"座って首が回る\"", "\"SitAndSpinHeadAction\""),
    ("\"固定\"", "\"Animate\""),
    ("\"楽に座る\"", "\"SitWithLegsUp\""),
    ("\"足を下ろして座る\"", "\"SitWithLegsDown\""),
    ("\"足をぶらぶらさせる\"", "\"SitAndDangleLegs\""),
    ("<!-- 寝そべる系 -->", "<!-- Laying -->"),
    ("\"寝そべる\"", "\"Sprawl\""),
    ("\"ずりずり\"", "\"Creep\""),
    ("<!-- 天井系 -->", "<!-- Ceiling -->"),
    ("\"天井に掴まる\"", "\"GrabCeiling\""),
    ("\"天井\"", "\"Ceiling\""),
    ("\"天井を伝う\"", "\"ClimbCeiling\""),
    ("<!-- 壁系 -->", "<!-- Wall -->"),
    ("\"壁に掴まる\"", "\"GrabWall\""),
    ("\"壁\"", "\"Wall\""),
    ("\"壁を登る\"", "\"ClimbWall\""),
    ("目的地Y", "TargetY"),
    ("目的地X", "TargetX"),
    ("<!-- IE系 -->", "<!-- IE -->"),
    ("\"IEを持って落ちる\"", "\"FallWithIe\""),
    ("IEの端X=", "IeOffsetX="),
    ("IEの端Y=", "IeOffsetY="),
    ("\"IEを持って歩く\"", "\"WalkWithIe\""),
    ("\"IEを持って走る\"", "\"RunWithIe\""),
    ("\"IEを投げる\"", "\"ThrowIe\""),
    ("初速X=", "InitialVX="),
    ("初速Y=", "InitialVY="),
    ("重力=", "Gravity="),
    ("<!-- 落下系 -->", "<!-- Falling -->"),
    ("\"ジャンプ\"", "\"Jumping\""),
    ("速度=", "VelocityParam="),
    ("\"落ちる\"", "\"Falling\""),
    ("空気抵抗X=", "RegistanceX="),
    ("空気抵抗Y=", "RegistanceY="),
    ("\"跳ねる\"", "\"Bouncing\""),
    ("\"転ぶ\"", "\"Tripping\""),
    ("<!-- ドラッグ系 -->", "<!-- Dragging -->"),
    ("\"つままれる\"", "\"Pinched\""),
    ("\"抵抗する\"", "\"Resisting\""),
    ("</動作リスト>", "</ActionList>"),
    ("<!-- 実際の行動 -->", "<!-- Actual Behavior -->"),
    ("<!-- システムが使用する -->", "<!-- ALWAYS REQUIRED -->"),
    ("\"落下する\"", "\"Fall\""),
    ("\"複合\"", "\"Sequence\""),
    ("繰り返し=", "Loop="),
    ("<動作参照 ", "<ActionReference "),
    ("\"選択\"", "\"Select\""),
    ("\"ドラッグされる\"", "\"Dragged\""),
    ("\"投げられる\"", "\"Thrown\""),
    ("\"立ってボーっとする\"", "\"StandUp\""),
    ("\"座ってボーっとする\"", "\"SitDown\""),
    ("\"寝そべってボーっとする\"", "\"LieDown\""),
    ("\"座って足をぶらぶらさせる\"", "\"SitWhileDanglingLegs\""),
    ("\"壁に掴まってボーっとする\"", "\"HoldOntoWall\""),
    ("\"壁から落ちる\"", "\"FallFromWall\""),
    ("\"天井に掴まってボーっとする\"", "\"HoldOntoCeiling\""),
    ("\"天井から落ちる\"", "\"FallFromCeiling\""),
    ("\"ワークエリアの下辺を歩く\"", "\"WalkAlongWorkAreaFloor\""),
    ("\"ワークエリアの下辺を走る\"", "\"RunAlongWorkAreaFloor\""),
    ("\"ワークエリアの下辺でずりずり\"", "\"CrawlAlongWorkAreaFloor\""),
    ("\"ワークエリアの下辺の左の端っこで座る\"", "\"WalkLeftAlongFloorAndSit\""),
    ("\"ワークエリアの下辺の右の端っこで座る\"", "\"WalkRightAlongFloorAndSit\""),
    ("右向き=", "LookRight="),
    ("\"ワークエリアの下辺から左の壁によじのぼる\"", "\"GrabWorkAreaBottomLeftWall\""),
    ("\"ワークエリアの下辺から右の壁によじのぼる\"", "\"GrabWorkAreaBottomRightWall\""),
    ("\"走ってワークエリアの下辺の左の端っこで座る\"", "\"WalkLeftAndSit\""),
    ("\"走ってワークエリアの下辺の右の端っこで座る\"", "\"WalkRightAndSit\""),
    ("\"走ってワークエリアの下辺から左の壁によじのぼる\"", "\"WalkAndGrabBottomLeftWall\""),
    ("\"走ってワークエリアの下辺から右の壁によじのぼる\"", "\"WalkAndGrabBottomRightWall\""),
    ("\"IEの下に飛びつく\"", "\"JumpFromBottomOfIE\""),
    ("\"ワークエリアの壁を途中まで登る\"", "\"ClimbHalfwayAlongWall\""),
    ("<行動リスト>", "<BehaviorList>"),
    ("<行動 ", "<Behavior "),
    ("頻度=", "Frequency="),
    ("<次の行動リスト ", "<NextBehavior "),
    ("追加=", "Add="),
    ("</次の行動リスト>", "</NextBehavior>"),
    ("</行動>", "</Behavior>"),
    ("\"マウスの周りに集まる\"", "\"ChaseMouse\""),
    ("<行動参照 ", "<BehaviorReference "),
    ("\"座ってマウスのほうを見る\"", "\"SitAndFaceMouse\""),
    ("\"座ってマウスのほうを見てたら首が回った\"", "\"SitAndSpinHead\""),
    ("\"引っこ抜かれる\"", "\"PullUp\""),
    ("\"分裂した\"", "\"Divided\""),
    ("<!-- 地面に接しているとき -->", "<!-- On the Floor -->"),
    ("<条件 ", "<Condition "),
    ("\"分裂する\"", "\"SplitIntoTwo\""),
    ("</条件>", "</Condition>"),
    ("<!-- 壁に接しているとき -->", "<!-- On the Wall -->"),
    ("<!-- 天井に接しているとき -->", "<!-- On the Ceiling -->"),
    ("<!-- ワークエリアの下辺に接しているとき -->", "<!-- On Work Area Floor -->"),
    ("<!-- ずりずりした後はそのままボーっとする -->", "<!-- Finished Crawling -->"),
    ("\"引っこ抜く\"", "\"PullUpShimeji\""),
    ("<!-- ワークエリアの壁に接しているとき -->", "<!-- On Work Area Facing the Wall -->"),
    ("\"ワークエリアの壁を登る\"", "\"ClimbAlongWall\""),
    ("<!-- ワークエリアの上辺に接しているとき -->", "<!-- On Work Area Top Facing -->"),
    ("\"ワークエリアの上辺を伝う\"", "\"ClimbAlongCeiling\""),
    ("<!-- IEの上辺に接しているとき -->", "<!-- On Top of IE -->"),
    ("\"IEの天井を歩く\"", "\"WalkAlongIECeiling\""),
    ("\"IEの天井を走る\"", "\"RunAlongIECeiling\""),
    ("\"IEの天井でずりずり\"", "\"CrawlAlongIECeiling\""),
    ("<!-- ずりずりした後はそのままボーっとする -->", "<!-- Finished Crawling -->"),
    ("\"IEの天井の左の端っこで座る\"", "\"SitOnTheLeftEdgeOfIE\""),
    ("\"IEの天井の右の端っこで座る\"", "\"SitOnTheRightEdgeOfIE\""),
    ("\"IEの天井の左の端っこから飛び降りる\"", "\"JumpFromLeftEdgeOfIE\""),
    ("\"IEの天井の右の端っこから飛び降りる\"", "\"JumpFromRightEdgeOfIE\""),
    ("\"走ってIEの天井の左の端っこで座る\"", "\"WalkLeftAlongIEAndSit\""),
    ("\"走ってIEの天井の右の端っこで座る\"", "\"WalkRightAlongIEAndSit\""),
    ("\"走ってIEの天井の左の端っこから飛び降りる\"", "\"WalkLeftAlongIEAndJump\""),
    ("\"走ってIEの天井の右の端っこから飛び降りる\"", "\"WalkRightAlongIEAndJump\""),
    ("<!-- IEの壁に接しているとき -->", "<!-- On IE's Side -->"),
    ("\"IEの壁を途中まで登る\"", "\"HoldOntoIEWall\""),
    ("\"IEの壁を登る\"", "\"ClimbIEWall\""),
    ("<!-- IEの下辺に接しているとき -->", "<!-- On the Bottom of IE -->"),
    ("\"IEの下辺を伝う\"", "\"ClimbIEBottom\""),
    ("\"IEの下辺から左の壁によじのぼる\"", "\"GrabIEBottomLeftWall\""),
    ("\"IEの下辺から右の壁によじのぼる\"", "\"GrabIEBottomRightWall\""),
    ("\"左の壁に飛びつく\"", "\"JumpFromLeftWall\""),
    ("\"右の壁に飛びつく\"", "\"JumpFromRightWall\""),
    ("<!-- IEが見えるとき -->", "<!-- IE Is Visible -->"),
    ("\"IEの左に飛びつく\"", "\"JumpOnIELeftWall\""),
    ("\"IEの右に飛びつく\"", "\"JumpOnIERightWall\""),
    ("\"IEを右に投げる\"", "\"ThrowIEFromLeft\""),
    ("\"IEを左に投げる\"", "\"ThrowIEFromRight\""),
    ("\"走ってIEを右に投げる\"", "\"WalkAndThrowIEFromRight\""),
    ("\"走ってIEを左に投げる\"", "\"WalkAndThrowIEFromLeft\""),
    ("</行動リスト>", "</BehaviorList>"),
    ("</マスコット>", "</Mascot>"),
    ("#{footX", "#{FootX"),
    ("\"猛ダッシュでIEの天井の左の端っこから飛び降りる\"", "\"DashIeCeilingLeftEdgeFromJump\""),
    ("\"猛ダッシュでIEの天井の右の端っこから飛び降りる\"", "\"DashIeCeilingRightEdgeFromJump\""),
    ("\"猛ダッシュでIEの天井の左の端っこから飛び降りる\"", "\"DashIeCeilingLeftEdgeFromJump\""),
    ("\"猛ダッシュでIEの天井の右の端っこから飛び降りる\"", "\"DashIeCeilingRightEdgeFromJump\""),
    ("ずれ=", "Gap="),
    ("+ずれ", "+Gap"),
    ("\"引っこ抜く1\"", "\"PullUpShimeji1\""),
    ("生まれる場所X=", "BornX="),
    ("生まれる場所Y=", "BornY="),
    ("生まれた時の行動=", "BornBehavior="),
    ("\"引っこ抜く2\"", "\"PullUpShimeji2\""),
    ("\"分裂1\"", "\"Divide1\""),
    // Algunos originales escriben `<条件` seguido de un salto de línea, no de un espacio.
    ("<条件", "<Condition"),
];

/// Aplica todas las sustituciones, en orden, sobre el XML original.
pub fn translate(xml: &str) -> String {
    let mut out = xml.to_string();
    for (japanese, english) in REPLACEMENTS {
        out = out.replace(japanese, english);
    }
    out
}

/// Marcadores que distinguen el XML de acciones del de comportamientos.
pub const ACTIONS_MARKER: &str = "<動作リスト>";
pub const BEHAVIORS_MARKER: &str = "<行動リスト>";

/// Detecta marcadores japoneses en un XML de Shimeji original.
pub fn is_japanese(xml: &str) -> bool {
    [
        "<マスコット", ACTIONS_MARKER, BEHAVIORS_MARKER, "<アニメーション", "<ポーズ ", "<行動 ",
    ]
    .iter()
    .any(|marker| xml.contains(marker))
}

pub fn is_actions(xml: &str) -> bool {
    xml.contains(ACTIONS_MARKER)
}

pub fn is_behaviors(xml: &str) -> bool {
    xml.contains(BEHAVIORS_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_markers_names_and_values() {
        let xml = "<?xml?>\n<マスコット xmlns=\"x\">\n<動作リスト>\n\
                   <動作 名前=\"振り向く\" 種類=\"組み込み\" クラス=\"com.Look\" />\n\
                   <アニメーション 画像=\"0.png\" 基準座標=\"0,0\" />\n\
                   </動作リスト>\n<行動リスト>\n<条件\n  Condition=\"1\" />\n\
                   <行動 名前=\"落ちる\" 頻度=\"1\" />\n</条件>\n\
                   </行動リスト>\n</マスコット>\n";
        let out = translate(xml);
        assert!(out.starts_with("<?xml?>\n<Mascot "));
        assert!(out.contains("<ActionList>"));
        assert!(out.contains("<Action Name=\"Look\" Type=\"Embedded\" Class=\"com.Look\" />"));
        assert!(out.contains("<Animation Image=\"0.png\" ImageAnchor=\"0,0\" />"));
        assert!(out.contains("<BehaviorList>"));
        assert!(out.contains("<Condition\n  Condition=\"1\" />"));
        assert!(out.contains("</Condition>"));
        assert!(out.contains("<Behavior Name=\"Falling\" Frequency=\"1\" />"));
        assert!(!is_japanese(&out));
    }

    #[test]
    fn detects_original_xml_kind() {
        assert!(is_actions("<動作リスト>"));
        assert!(is_behaviors("<行動リスト>"));
        assert!(is_japanese("<マスコット "));
        assert!(!is_japanese("<Mascot><ActionList></ActionList></Mascot>"));
    }
}
