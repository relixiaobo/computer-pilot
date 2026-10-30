// Compile as main.swift alongside upstream CursorMotionModel.swift at c6ffbce2.
// Run the executable into visual-cursor-reference.json. See the research note.
import Foundation
import CoreGraphics
func visualCursorScreenStateVelocity(fromRuntimeVelocity v: CGVector, yAxisMultiplier: CGFloat) -> CGVector {
    CGVector(dx: v.dx, dy: v.dy * yAxisMultiplier)
}
let bounds = CGRect(x: 0, y: 0, width: 1280, height: 800)
let routes: [(CGPoint, CGPoint)] = [
    (CGPoint(x:100,y:120),CGPoint(x:720,y:380)),
    (CGPoint(x:720,y:380),CGPoint(x:100,y:120)),
    (CGPoint(x:120,y:620),CGPoint(x:960,y:140)),
    (CGPoint(x:960,y:620),CGPoint(x:120,y:140)),
    (CGPoint(x:640,y:400),CGPoint(x:641,y:401)),
    (CGPoint(x:0,y:0),CGPoint(x:1279,y:799)),
    (CGPoint(x:640,y:400),CGPoint(x:640,y:400))
]
let paths: [[String: Any]] = routes.map { start, end in
    let candidates = OfficialCursorMotionModel.makeCandidates(start:start,end:end,bounds:bounds)
    let c = OfficialCursorMotionModel.chooseBestCandidate(from:candidates)!
    return ["start":[start.x,start.y],"end":[end.x,end.y],"id":c.identifier,"score":c.score,
        "points":[0.0,0.1,0.25,0.5,0.75,0.9,1.0].map { t in let p=c.path.point(at:t); return [p.x,p.y] }]
}
var spring = CursorMotionSpringState()
var progress: CGFloat = 0
var springSamples: [[CGFloat]] = []
for t: CGFloat in [0.0,0.1,0.25,0.5,0.75,1.0,1.25,1.429166666666663] {
    (progress,spring) = CursorMotionProgressAnimator.advance(current:progress,state:spring,to:t)
    springSamples.append([t,progress])
}
let output: [String: Any] = ["source":"open-codex-computer-use c6ffbce2fc49b2a6a42b7c7acac31014efc6b1e2 CursorMotionModel.swift", "bounds":[0,0,1280,800],"paths":paths,"spring":springSamples]
FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject:output,options:[.prettyPrinted,.sortedKeys]))
