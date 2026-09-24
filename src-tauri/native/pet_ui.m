#import <Cocoa/Cocoa.h>
#import <objc/runtime.h>

// This file only handles explicit pet interactions. It never reads another
// application's titles, controls, clipboard, or screen pixels. The speech
// bubble itself is a web page (src/bubble); this only makes its window, and
// the pet overlay's, behave as auxiliary panels.
typedef void (*PerchCallback)(const char *);
static PerchCallback callback;
static void emit(NSDictionary *value) {
  NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
  if (data && callback) callback([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
}

@interface PerchOverlayPanel : NSPanel @end
@implementation PerchOverlayPanel
- (BOOL)canBecomeKeyWindow { return NO; }
- (BOOL)canBecomeMainWindow { return NO; }
@end
/// Takes keyboard input without ever becoming the main window, and without
/// activating Perch: typing into the bubble leaves the frontmost app in front.
@interface PerchBubblePanel : NSPanel @end
@implementation PerchBubblePanel
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

@interface PerchDrag : NSObject <NSFilePromiseProviderDelegate, NSDraggingSource>
@property NSNumber *pet;
@property NSString *filename;
@property BOOL promised;
@end
static NSMutableSet *drags;
@implementation PerchDrag
- (NSString *)filePromiseProvider:(NSFilePromiseProvider *)provider fileNameForType:(NSString *)type { return _filename; }
- (void)filePromiseProvider:(NSFilePromiseProvider *)provider writePromiseToURL:(NSURL *)url completionHandler:(void (^)(NSError *))completion {
  // A hidden, unique handoff file satisfies Finder's native promise protocol.
  // It is removed after Finder completes the transfer; no user file is replaced.
  NSError *error = nil;
  NSData *data = [@"Perch folder handoff\n" dataUsingEncoding:NSUTF8StringEncoding];
  BOOL wrote = [data writeToURL:url options:NSDataWritingWithoutOverwriting error:&error];
  completion(error);
  if (wrote) {
    self.promised = YES;
    emit(@{@"type": @"bind", @"petId": self.pet, @"folder": url.URLByDeletingLastPathComponent.path, @"handoff": url.path});
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), dispatch_get_main_queue(), ^{ [[NSFileManager defaultManager] removeItemAtURL:url error:nil]; [drags removeObject:self]; });
  } else { emit(@{@"type": @"error", @"message": @"Finder could not complete the folder drop. Try a writable folder."}); }
}
- (NSOperationQueue *)operationQueueForFilePromiseProvider:(NSFilePromiseProvider *)provider { return NSOperationQueue.mainQueue; }
- (NSDragOperation)draggingSession:(NSDraggingSession *)session sourceOperationMaskForDraggingContext:(NSDraggingContext)context { return NSDragOperationCopy; }
- (void)draggingSession:(NSDraggingSession *)session endedAtPoint:(NSPoint)point operation:(NSDragOperation)operation {
  emit(@{@"type": @"dragEnded", @"petId": self.pet, @"x": @(point.x), @"y": @(point.y), @"accepted": @(operation != NSDragOperationNone)});
  if (operation == NSDragOperationNone) [drags removeObject:self];
}
@end
void perch_ui_init(PerchCallback cb) { callback = cb; }
void perch_overlay_panel(void *pointer) {
  NSWindow *window = (__bridge NSWindow *)pointer;
  // Match the native nonactivating panel pattern used by macOS tray utilities.
  object_setClass(window, PerchOverlayPanel.class);
  window.styleMask |= NSWindowStyleMaskNonactivatingPanel;
  ((NSPanel *)window).hidesOnDeactivate = NO;
  window.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorIgnoresCycle;
}
void perch_bubble_panel(void *pointer) {
  NSWindow *window = (__bridge NSWindow *)pointer;
  object_setClass(window, PerchBubblePanel.class);
  window.styleMask |= NSWindowStyleMaskNonactivatingPanel;
  NSPanel *panel = (NSPanel *)window;
  panel.hidesOnDeactivate = NO;
  panel.becomesKeyOnlyIfNeeded = NO;
  panel.level = NSFloatingWindowLevel;
  window.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorIgnoresCycle;
}
static NSView *perch_find_webview(NSView *view) {
  Class webview = NSClassFromString(@"WKWebView");
  if (webview && [view isKindOfClass:webview]) return view;
  for (NSView *child in view.subviews) {
    NSView *found = perch_find_webview(child);
    if (found) return found;
  }
  return nil;
}
/// Front and key, without activating the application — the part the toolkit's
/// own focus call cannot do, since it activates the app first.
void perch_bubble_focus(void *pointer) {
  NSWindow *window = (__bridge NSWindow *)pointer;
  [window orderFrontRegardless];
  [window makeKeyWindow];
  NSView *webview = perch_find_webview(window.contentView);
  if (webview) [window makeFirstResponder:webview];
}
int perch_begin_folder_drag(void *pointer, unsigned long long pet, const char *image, int frames) {
  NSWindow *window = (__bridge NSWindow *)pointer;
  if (!(NSEvent.pressedMouseButtons & 1)) return 0;
  NSEvent *event = [NSEvent mouseEventWithType:NSEventTypeLeftMouseDragged location:[window convertPointFromScreen:NSEvent.mouseLocation] modifierFlags:0 timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:window.windowNumber context:nil eventNumber:0 clickCount:1 pressure:1];
  PerchDrag *source = [PerchDrag new];
  source.pet = @(pet);
  source.filename = [NSString stringWithFormat:@".perch-handoff-%@", NSUUID.UUID.UUIDString];
  if (!drags) drags = [NSMutableSet new];
  [drags addObject:source];
  NSFilePromiseProvider *provider = [[NSFilePromiseProvider alloc] initWithFileType:@"public.data" delegate:source];
  NSDraggingItem *item = [[NSDraggingItem alloc] initWithPasteboardWriter:provider];
  NSString *encoded = [NSString stringWithUTF8String:image];
  NSString *base64 = [[encoded componentsSeparatedByString:@","] lastObject];
  NSImage *atlas = [[NSImage alloc] initWithData:[[NSData alloc] initWithBase64EncodedString:base64 options:0]];
  NSImage *sprite = [[NSImage alloc] initWithSize:NSMakeSize(80, 80)];
  [sprite lockFocus];
  [[NSGraphicsContext currentContext] setImageInterpolation:NSImageInterpolationNone];
  [atlas drawInRect:NSMakeRect(0, 0, 80, 80) fromRect:NSMakeRect(0, 0, atlas.size.width / MAX(frames, 1), atlas.size.height) operation:NSCompositingOperationSourceOver fraction:1];
  [sprite unlockFocus];
  NSView *view = window.contentView;
  NSPoint point = [view convertPoint:[window convertPointFromScreen:NSEvent.mouseLocation] fromView:nil];
  [item setDraggingFrame:NSMakeRect(point.x - 40, point.y - 40, 80, 80) contents:sprite];
  [view beginDraggingSessionWithItems:@[item] event:event source:source];
  return 1;
}