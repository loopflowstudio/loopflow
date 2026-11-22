import Loopflow
import SwiftUI

/// iPhone's sizes in the shared faces. The Mac ramp is set for a 13 pt desktop;
/// iPhone keeps its own sizes until it gets a ramp of its own.
enum MobileType {
    static func text(_ size: CGFloat) -> Font {
        .custom(Typography.sansFace, size: size)
    }

    static func strong(_ size: CGFloat) -> Font {
        .custom(Typography.sansStrongFace, size: size)
    }
}
