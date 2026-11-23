#import "ApaKanAppDelegate.h"

#import <objc/runtime.h>

void apa_kan_push_deep_link(const char *url);

static void apakan_push_nsurl(NSURL *url) {
	if (url == nil) {
		return;
	}
	NSString *absolute = url.absoluteString;
	if (absolute.length == 0) {
		return;
	}
	apa_kan_push_deep_link(absolute.UTF8String);
}

@implementation ApaKanAppDelegate

- (BOOL)application:(UIApplication *)application
	didFinishLaunchingWithOptions:(NSDictionary<UIApplicationLaunchOptionsKey, id> *)launchOptions {
	(void)application;
	NSURL *url = launchOptions[UIApplicationLaunchOptionsURLKey];
	if (url != nil) {
		apakan_push_nsurl(url);
	}
	NSUserActivity *activity = launchOptions[UIApplicationLaunchOptionsUserActivityDictionaryKey][@"UIApplicationLaunchOptionsUserActivityKey"];
	if ([activity isKindOfClass:[NSUserActivity class]] && activity.webpageURL != nil) {
		apakan_push_nsurl(activity.webpageURL);
	}
	return YES;
}

- (BOOL)application:(UIApplication *)app
			openURL:(NSURL *)url
			options:(NSDictionary<UIApplicationOpenURLOptionsKey, id> *)options {
	(void)app;
	(void)options;
	apakan_push_nsurl(url);
	return YES;
}

- (BOOL)application:(UIApplication *)application
	continueUserActivity:(NSUserActivity *)userActivity
	 restorationHandler:(void (^)(NSArray<id<UIUserActivityRestoring>> *_Nullable))restorationHandler {
	(void)application;
	(void)restorationHandler;
	if ([userActivity.activityType isEqualToString:NSUserActivityTypeBrowsingWeb]) {
		apakan_push_nsurl(userActivity.webpageURL);
		return userActivity.webpageURL != nil;
	}
	return NO;
}

@end

static void apakan_install_delegate(UIApplication *app) {
	static ApaKanAppDelegate *delegate;
	if (delegate == nil) {
		delegate = [ApaKanAppDelegate new];
	}
	app.delegate = delegate;
}

@interface UIApplication (ApaKanDeepLinks)
- (instancetype)apakan_init;
@end

@implementation UIApplication (ApaKanDeepLinks)

+ (void)load {
	Method orig = class_getInstanceMethod(self, @selector(init));
	Method hook = class_getInstanceMethod(self, @selector(apakan_init));
	if (orig && hook) {
		method_exchangeImplementations(orig, hook);
	}
}

- (instancetype)apakan_init {
	UIApplication *app = [self apakan_init];
	if (app != nil) {
		apakan_install_delegate(app);
	}
	return app;
}

@end
