#pragma once

#import <UIKit/UIKit.h>

/// Thin UIApplicationDelegate compiled into the Bevy binary.
///
/// No SwiftUI. Forwards `apakan://` and associated-domain universal links into
/// `apa_kan_push_deep_link` → [`IncomingUrl`] (`apa-kan/game/src/platform.rs`).
@interface ApaKanAppDelegate : UIResponder <UIApplicationDelegate>
@end
