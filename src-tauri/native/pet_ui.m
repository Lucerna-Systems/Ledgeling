#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>

// This file only handles explicit pet interactions. It never reads another
// application's titles, controls, clipboard, or screen pixels.
typedef void (*PerchCallback)(const char *);
static PerchCallback callback;
static void emit(NSDictionary *value) {
  NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
  if (data && callback) callback([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
}

// The bubble mirrors the web companion's palette (src/style.css and
// src/assistant/assistant.css) so the native surface reads as the same product.
static NSColor *perch_color(int hex) {
  return [NSColor colorWithSRGBRed:((hex >> 16) & 255) / 255.0 green:((hex >> 8) & 255) / 255.0 blue:(hex & 255) / 255.0 alpha:1];
}

/// One in-flight tween, keyed by the object it animates so a new animation for
/// the same control replaces the old one.
@interface PerchTween : NSObject
@property (nonatomic, strong) NSTimer *timer;
@property (nonatomic, copy) void (^step)(CGFloat);
@property (nonatomic, copy) void (^done)(void);
@end
@implementation PerchTween
@end

static NSMutableDictionary<NSValue *, PerchTween *> *perch_tweens;

/// Stops the owner's tween without running its completion — which is what lets
/// a re-open cancel a closing fade without the panel being ordered out anyway.
static void perch_stop(id owner) {
  if (!perch_tweens) return;
  NSValue *key = [NSValue valueWithPointer:(__bridge const void *)owner];
  [perch_tweens[key].timer invalidate];
  [perch_tweens removeObjectForKey:key];
}

static BOOL perch_reduce_motion(void) {
  return NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion;
}

/// Timer-driven ease-out tween. `step` receives eased progress 0→1; `done` runs
/// only when the tween finishes on its own. The bubble's controls animate custom
/// drawRect properties, which CAAnimation cannot reach.
static void perch_animate(id owner, NSTimeInterval duration, void (^step)(CGFloat), void (^done)(void)) {
  perch_stop(owner);
  if (perch_reduce_motion() || duration <= 0) {
    step(1);
    if (done) done();
    return;
  }
  if (!perch_tweens) perch_tweens = [NSMutableDictionary new];
  PerchTween *tween = [PerchTween new];
  tween.step = step;
  tween.done = done;
  NSValue *key = [NSValue valueWithPointer:(__bridge const void *)owner];
  perch_tweens[key] = tween;
  NSDate *start = [NSDate date];
  __weak PerchTween *weak = tween;
  tween.timer = [NSTimer timerWithTimeInterval:1.0 / 60.0 repeats:YES block:^(NSTimer *timer) {
    __strong PerchTween *strong = weak;
    if (!strong) { [timer invalidate]; return; }
    CGFloat raw = (CGFloat)(-[start timeIntervalSinceNow]) / duration;
    strong.step(raw >= 1 ? 1 : 1 - powf(1 - raw, 3));
    if (raw >= 1) {
      [timer invalidate];
      if (perch_tweens[key] == strong) [perch_tweens removeObjectForKey:key];
      if (strong.done) strong.done();
    }
  }];
  [[NSRunLoop mainRunLoop] addTimer:tween.timer forMode:NSRunLoopCommonModes];
  tween.step(0);
}

/// The hover controls all draw their own hover wash, so one helper animates
/// every one of them.
@protocol PerchHovering <NSObject>
@property (nonatomic) CGFloat hover;
@end
static void perch_hover_to(NSObject<PerchHovering> *control, CGFloat target) {
  CGFloat from = control.hover;
  perch_animate(control, 0.14, ^(CGFloat t) { control.hover = from + (target - from) * t; }, nil);
}

@interface PerchOverlayPanel : NSPanel @end
@implementation PerchOverlayPanel
- (BOOL)canBecomeKeyWindow { return NO; }
- (BOOL)canBecomeMainWindow { return NO; }
@end
@interface PerchBubblePanel : NSPanel @end
@implementation PerchBubblePanel
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }
@end
// The whole bubble outline, tail included, as one closed path: the border then
// carries across the tail's opening instead of cutting the bubble in two, and
// the same shape masks the blur behind it. `tailX` is where the tail points in
// view-local x — set from the anchor (mouse/pet) each time the bubble is
// positioned, so it follows the pet instead of sitting at a fixed spot.
// `tailUp` flips it to the top edge, for when the panel had to go below.
static NSBezierPath *perch_bubble_path(NSRect bounds, CGFloat tailX, BOOL tailUp) {
  CGFloat w = bounds.size.width, h = bounds.size.height, r = 20, notch = 16, half = 14;
  CGFloat corner = r * 0.55;
  CGFloat tip = MAX(r + half + 4, MIN(tailX, w - r - half - 4));
  NSBezierPath *shape = [NSBezierPath bezierPath];
  if (tailUp) {
    CGFloat yt = h - notch;
    [shape moveToPoint:NSMakePoint(tip + half, yt)];
    [shape lineToPoint:NSMakePoint(tip, h)];
    [shape lineToPoint:NSMakePoint(tip - half, yt)];
    [shape lineToPoint:NSMakePoint(r, yt)];
    [shape curveToPoint:NSMakePoint(0, yt - r) controlPoint1:NSMakePoint(corner, yt) controlPoint2:NSMakePoint(0, yt - corner)];
    [shape lineToPoint:NSMakePoint(0, r)];
    [shape curveToPoint:NSMakePoint(r, 0) controlPoint1:NSMakePoint(0, corner) controlPoint2:NSMakePoint(corner, 0)];
    [shape lineToPoint:NSMakePoint(w - r, 0)];
    [shape curveToPoint:NSMakePoint(w, r) controlPoint1:NSMakePoint(w - corner, 0) controlPoint2:NSMakePoint(w, corner)];
    [shape lineToPoint:NSMakePoint(w, yt - r)];
    [shape curveToPoint:NSMakePoint(w - r, yt) controlPoint1:NSMakePoint(w, yt - corner) controlPoint2:NSMakePoint(w - corner, yt)];
  } else {
    CGFloat yb = notch;
    [shape moveToPoint:NSMakePoint(tip - half, yb)];
    [shape lineToPoint:NSMakePoint(tip, 0)];
    [shape lineToPoint:NSMakePoint(tip + half, yb)];
    [shape lineToPoint:NSMakePoint(w - r, yb)];
    [shape curveToPoint:NSMakePoint(w, yb + r) controlPoint1:NSMakePoint(w - corner, yb) controlPoint2:NSMakePoint(w, yb + corner)];
    [shape lineToPoint:NSMakePoint(w, h - r)];
    [shape curveToPoint:NSMakePoint(w - r, h) controlPoint1:NSMakePoint(w, h - corner) controlPoint2:NSMakePoint(w - corner, h)];
    [shape lineToPoint:NSMakePoint(r, h)];
    [shape curveToPoint:NSMakePoint(0, h - r) controlPoint1:NSMakePoint(corner, h) controlPoint2:NSMakePoint(0, h - corner)];
    [shape lineToPoint:NSMakePoint(0, yb + r)];
    [shape curveToPoint:NSMakePoint(r, yb) controlPoint1:NSMakePoint(0, yb + corner) controlPoint2:NSMakePoint(corner, yb)];
  }
  [shape closePath];
  return shape;
}

/// Tint and hairline, drawn over the blur. Decorative only, so `hitTest:`
/// returns nil and clicks reach the controls layered above it.
@interface PerchBubbleChrome : NSView
@property (nonatomic) CGFloat tailX;
@property (nonatomic) BOOL tailUp;
@end
@implementation PerchBubbleChrome
- (NSView *)hitTest:(NSPoint)point { return nil; }
- (void)drawRect:(NSRect)dirty {
  NSBezierPath *shape = perch_bubble_path(self.bounds, self.tailX, self.tailUp);
  // Light wash over the vibrancy: enough to hold the palette and keep text
  // legible, thin enough that the desktop still reads through.
  [[perch_color(0xfffef9) colorWithAlphaComponent:0.32] setFill];
  [shape fill];
  shape.lineWidth = 1;
  [[perch_color(0xffffff) colorWithAlphaComponent:0.5] setStroke];
  [shape stroke];
}
@end

@interface PerchBubbleView : NSView
@property (nonatomic) CGFloat tailX;
@property (nonatomic) BOOL tailUp;
@end
@implementation PerchBubbleView {
  NSVisualEffectView *_blur;
  PerchBubbleChrome *_chrome;
}
- (instancetype)initWithFrame:(NSRect)frame {
  self = [super initWithFrame:frame];
  if (!self) return nil;
  _tailX = frame.size.width / 2;
  _blur = [[NSVisualEffectView alloc] initWithFrame:self.bounds];
  _blur.material = NSVisualEffectMaterialMenu;
  _blur.blendingMode = NSVisualEffectBlendingModeBehindWindow;
  _blur.state = NSVisualEffectStateActive;
  _blur.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  [self addSubview:_blur];
  _chrome = [[PerchBubbleChrome alloc] initWithFrame:self.bounds];
  _chrome.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  [self addSubview:_chrome];
  [self refreshShape];
  return self;
}
- (void)setTailX:(CGFloat)tailX { _tailX = tailX; [self refreshShape]; }
- (void)setTailUp:(BOOL)tailUp { _tailUp = tailUp; [self refreshShape]; }
- (void)setFrameSize:(NSSize)size {
  [super setFrameSize:size];
  // The mask is cut to a fixed size; without recutting it on every resize the
  // blur stays clipped to the old outline while the border draws the new one.
  [self refreshShape];
}
- (void)refreshShape {
  CGFloat tailX = _tailX;
  BOOL tailUp = _tailUp;
  // The blur is clipped to the bubble outline; without the mask it would fill
  // the panel's whole rectangle and the rounded shape would disappear.
  _blur.maskImage = [NSImage imageWithSize:self.bounds.size flipped:NO drawingHandler:^BOOL(NSRect rect) {
    [NSColor.blackColor setFill];
    [perch_bubble_path(rect, tailX, tailUp) fill];
    return YES;
  }];
  _chrome.tailX = tailX;
  _chrome.tailUp = tailUp;
  [_chrome setNeedsDisplay:YES];
  // The window shadow is cached from the drawn alpha, so a new tail needs it.
  [self.window invalidateShadow];
}
@end

@interface PerchButton : NSControl <PerchHovering>
@property (nonatomic, copy) NSString *title;
@property (nonatomic) BOOL primary;
/// Hover eased over ~0.14s: fast enough to stay attached to the pointer,
/// but a wash rather than a snap.
@property (nonatomic) CGFloat hover;
@end
@implementation PerchButton {
  BOOL _pressed;
}
- (instancetype)initWithFrame:(NSRect)frame title:(NSString *)title primary:(BOOL)primary target:(id)target action:(SEL)action {
  self = [super initWithFrame:frame];
  if (!self) return nil;
  _title = [title copy];
  _primary = primary;
  self.target = target;
  self.action = action;
  [self setAccessibilityRole:NSAccessibilityButtonRole];
  [self setAccessibilityLabel:title];
  [self addTrackingArea:[[NSTrackingArea alloc] initWithRect:self.bounds options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways | NSTrackingInVisibleRect owner:self userInfo:nil]];
  return self;
}
- (void)setTitle:(NSString *)title {
  _title = [title copy];
  [self setAccessibilityLabel:title];
  [self setNeedsDisplay:YES];
}
- (void)setHover:(CGFloat)hover {
  _hover = hover;
  [self setNeedsDisplay:YES];
}
- (void)drawRect:(NSRect)dirty {
  CGFloat alpha = self.isEnabled ? 1 : 0.45;
  CGFloat wash = self.isEnabled ? _hover : 0;
  NSColor *fill;
  NSColor *text;
  NSColor *border = _primary ? nil : perch_color(0xd7ddce);
  // Secondary buttons stay part-transparent so the blur reads through them.
  CGFloat opacity = alpha;
  if (_primary) {
    fill = [perch_color(0x57704a) blendedColorWithFraction:wash ofColor:perch_color(0x415a37)];
    text = perch_color(0xf9faf5);
  } else {
    fill = [perch_color(0xf6f8ef) blendedColorWithFraction:wash ofColor:perch_color(0xe4ead6)];
    text = perch_color(0x3e5237);
    opacity = alpha * (0.68 + 0.24 * wash);
  }
  NSRect body = NSInsetRect(self.bounds, 0.5, 0.5);
  if (_pressed) body.origin.y -= 1;  // the web buttons' active translateY(1px)
  CGFloat radius = body.size.height / 2;  // pill, matching the segmented control
  NSBezierPath *shape = [NSBezierPath bezierPathWithRoundedRect:body xRadius:radius yRadius:radius];
  [[fill colorWithAlphaComponent:opacity] setFill];
  [shape fill];
  if (border) {
    [[border colorWithAlphaComponent:(0.75 + 0.25 * wash) * alpha] setStroke];
    [shape stroke];
  }
  NSDictionary *attrs = @{
    NSFontAttributeName: [NSFont systemFontOfSize:12 weight:NSFontWeightMedium],
    NSForegroundColorAttributeName: [text colorWithAlphaComponent:alpha],
  };
  [_title drawAtPoint:NSMakePoint(body.origin.x + (body.size.width - [_title sizeWithAttributes:attrs].width) / 2,
                                  body.origin.y + (body.size.height - [_title sizeWithAttributes:attrs].height) / 2)
        withAttributes:attrs];
}
- (void)mouseEntered:(NSEvent *)event { perch_hover_to(self, 1); }
- (void)mouseExited:(NSEvent *)event { perch_hover_to(self, 0); }
- (void)mouseDown:(NSEvent *)event {
  if (!self.isEnabled) return;
  _pressed = YES;
  [self setNeedsDisplay:YES];
  for (;;) {
    NSEvent *next = [NSApp nextEventMatchingMask:NSEventMaskLeftMouseDragged | NSEventMaskLeftMouseUp
                                      untilDate:[NSDate distantFuture]
                                         inMode:NSEventTrackingRunLoopMode dequeue:YES];
    BOOL inside = NSPointInRect([self convertPoint:next.locationInWindow fromView:nil], self.bounds);
    if (next.type == NSEventTypeLeftMouseUp) {
      _pressed = NO;
      [self setNeedsDisplay:YES];
      if (inside && self.action) [NSApp sendAction:self.action to:self.target from:self];
      return;
    }
    if (inside != _pressed) {
      _pressed = inside;
      [self setNeedsDisplay:YES];
    }
  }
}
@end

@interface PerchSegmented : NSControl
@property (nonatomic) NSInteger selectedSegment;
/// The pill's fractional index, eased to the selected segment so the slide is
/// visible; labels recolour against it (see drawRect).
@property (nonatomic) CGFloat pill;
- (instancetype)initWithFrame:(NSRect)frame labels:(NSArray<NSString *> *)labels target:(id)target action:(SEL)action;
/// Width that fits every label; equal thirds clip the longest one.
- (CGFloat)naturalWidth;
@end
@implementation PerchSegmented {
  NSArray<NSString *> *_labels;
  NSArray<NSNumber *> *_widths;
  NSArray<NSNumber *> *_offsets;
  CGFloat _natural;
}
- (instancetype)initWithFrame:(NSRect)frame labels:(NSArray<NSString *> *)labels target:(id)target action:(SEL)action {
  self = [super initWithFrame:frame];
  if (!self) return nil;
  _labels = [labels copy];
  _selectedSegment = 0;
  _pill = 0;
  self.target = target;
  self.action = action;
  [self setAccessibilityRole:NSAccessibilityTabGroupRole];
  [self setAccessibilityLabel:[labels componentsJoinedByString:@", "]];
  NSMutableArray *widths = [NSMutableArray new], *offsets = [NSMutableArray new];
  CGFloat x = 0;
  for (NSString *label in _labels) {
    CGFloat w = ceil([label sizeWithAttributes:@{NSFontAttributeName: [NSFont systemFontOfSize:12 weight:NSFontWeightMedium]}].width) + 30;
    [offsets addObject:@(x)];
    [widths addObject:@(w)];
    x += w;
  }
  _widths = widths;
  _offsets = offsets;
  _natural = x;
  return self;
}
- (CGFloat)naturalWidth { return _natural; }
/// The pill tracks a fractional index, so it has to interpolate both edges of
/// segments that are not the same width.
- (void)pillRect:(NSRect *)out {
  NSInteger last = (NSInteger)_labels.count - 1;
  NSInteger i = MAX(0, MIN((NSInteger)floor(_pill), last));
  NSInteger j = MIN(i + 1, last);
  CGFloat f = _pill - (CGFloat)i;
  CGFloat x = _offsets[i].doubleValue + (_offsets[j].doubleValue - _offsets[i].doubleValue) * f;
  CGFloat w = _widths[i].doubleValue + (_widths[j].doubleValue - _widths[i].doubleValue) * f;
  *out = NSMakeRect(x, 0, w, NSHeight(self.bounds));
}
- (void)setPill:(CGFloat)pill {
  _pill = pill;
  [self setNeedsDisplay:YES];
}
- (void)drawRect:(NSRect)dirty {
  NSRect inset = NSInsetRect(self.bounds, 0.5, 0.5);
  [[perch_color(0xd9dfd1) colorWithAlphaComponent:0.62] setFill];
  [[NSBezierPath bezierPathWithRoundedRect:inset xRadius:inset.size.height / 2 yRadius:inset.size.height / 2] fill];
  NSRect pill;
  [self pillRect:&pill];
  pill = NSInsetRect(pill, 2.5, 2.5);
  [[perch_color(0xffffff) colorWithAlphaComponent:0.92] setFill];
  [[NSBezierPath bezierPathWithRoundedRect:pill xRadius:NSHeight(pill) / 2 yRadius:NSHeight(pill) / 2] fill];
  for (NSInteger i = 0; i < (NSInteger)_labels.count; i++) {
    // Label colour tracks how close the pill actually is, so the text warms
    // up in step with the slide rather than flipping at the click.
    CGFloat on = MAX(0, 1 - fabs(_pill - (CGFloat)i));
    NSDictionary *attrs = @{
      NSFontAttributeName: [NSFont systemFontOfSize:12 weight:on > 0.5 ? NSFontWeightMedium : NSFontWeightRegular],
      NSForegroundColorAttributeName: [perch_color(0x737c6d) blendedColorWithFraction:on ofColor:perch_color(0x3d563b)],
    };
    NSString *title = _labels[i];
    NSSize size = [title sizeWithAttributes:attrs];
    [title drawAtPoint:NSMakePoint(_offsets[i].doubleValue + (_widths[i].doubleValue - size.width) / 2,
                                   (NSHeight(self.bounds) - size.height) / 2)
        withAttributes:attrs];
  }
}
- (void)mouseDown:(NSEvent *)event {
  NSPoint point = [self convertPoint:event.locationInWindow fromView:nil];
  NSInteger segment = 0;
  for (NSInteger i = 0; i < (NSInteger)_labels.count; i++) {
    if (point.x >= _offsets[i].doubleValue) segment = i;
  }
  if (segment != _selectedSegment) {
    _selectedSegment = segment;
    CGFloat from = _pill;
    CGFloat target = (CGFloat)segment;
    perch_animate(self, 0.22, ^(CGFloat t) { self.pill = from + (target - from) * t; }, nil);
    if (self.action) [NSApp sendAction:self.action to:self.target from:self];
  }
}
@end

@interface PerchCheck : NSControl
@property (nonatomic) BOOL on;
/// 0-1 fill/tick state.
@property (nonatomic) CGFloat mark;
- (instancetype)initWithFrame:(NSRect)frame title:(NSString *)title target:(id)target action:(SEL)action;
@end
@implementation PerchCheck {
  NSString *_title;
}
- (instancetype)initWithFrame:(NSRect)frame title:(NSString *)title target:(id)target action:(SEL)action {
  self = [super initWithFrame:frame];
  if (!self) return nil;
  _title = [title copy];
  self.target = target;
  self.action = action;
  [self setAccessibilityRole:NSAccessibilityCheckBoxRole];
  [self setAccessibilityLabel:title];
  return self;
}
- (void)setMark:(CGFloat)mark {
  _mark = mark;
  [self setNeedsDisplay:YES];
}
- (void)drawRect:(NSRect)dirty {
  CGFloat side = 15;
  CGFloat y = (self.bounds.size.height - side) / 2;
  NSBezierPath *box = [NSBezierPath bezierPathWithRoundedRect:NSInsetRect(NSMakeRect(0, y, side, side), 0.5, 0.5) xRadius:5 yRadius:5];
  [[[perch_color(0xffffff) colorWithAlphaComponent:0.75] blendedColorWithFraction:_mark ofColor:perch_color(0x779267)] setFill];
  [box fill];
  [[perch_color(0xd7ddce) colorWithAlphaComponent:0.9] setStroke];
  [box stroke];
  if (_mark > 0.01) {
    NSBezierPath *tick = [NSBezierPath bezierPath];
    [tick moveToPoint:NSMakePoint(3.5, y + 7.5)];
    [tick lineToPoint:NSMakePoint(6.2, y + 4.8)];
    [tick lineToPoint:NSMakePoint(11.5, y + 10.5)];
    [[perch_color(0xffffff) colorWithAlphaComponent:_mark] setStroke];
    tick.lineWidth = 2;
    tick.lineCapStyle = NSLineCapStyleRound;
    tick.lineJoinStyle = NSLineJoinStyleRound;
    [tick stroke];
  }
  NSDictionary *attrs = @{
    NSFontAttributeName: [NSFont systemFontOfSize:11.5],
    NSForegroundColorAttributeName: perch_color(0x4d5949),
  };
  [_title drawAtPoint:NSMakePoint(23, y + (side - [_title sizeWithAttributes:attrs].height) / 2) withAttributes:attrs];
}
- (void)mouseDown:(NSEvent *)event {
  self.on = !self.on;
  CGFloat from = _mark;
  CGFloat target = self.on ? 1 : 0;
  perch_animate(self, 0.18, ^(CGFloat t) { self.mark = from + (target - from) * t; }, nil);
  if (self.action) [NSApp sendAction:self.action to:self.target from:self];
}
@end

/// A rectangle with independent corner radii at its low and high y edges, so a
/// control and the list dropping out of it can meet on a shared square edge.
static NSBezierPath *perch_capsule(NSRect r, CGFloat lowR, CGFloat highR) {
  CGFloat x = NSMinX(r), y = NSMinY(r), w = NSWidth(r), h = NSHeight(r);
  CGFloat lk = lowR * 0.55, hk = highR * 0.55;
  NSBezierPath *p = [NSBezierPath bezierPath];
  [p moveToPoint:NSMakePoint(x, y + lowR)];
  if (lowR > 0)
    [p curveToPoint:NSMakePoint(x + lowR, y) controlPoint1:NSMakePoint(x, y + lk) controlPoint2:NSMakePoint(x + lk, y)];
  [p lineToPoint:NSMakePoint(x + w - lowR, y)];
  if (lowR > 0)
    [p curveToPoint:NSMakePoint(x + w, y + lowR) controlPoint1:NSMakePoint(x + w - lk, y) controlPoint2:NSMakePoint(x + w, y + lk)];
  [p lineToPoint:NSMakePoint(x + w, y + h - highR)];
  if (highR > 0)
    [p curveToPoint:NSMakePoint(x + w - highR, y + h) controlPoint1:NSMakePoint(x + w, y + h - hk) controlPoint2:NSMakePoint(x + w - hk, y + h)];
  [p lineToPoint:NSMakePoint(x + highR, y + h)];
  if (highR > 0)
    [p curveToPoint:NSMakePoint(x, y + h - highR) controlPoint1:NSMakePoint(x + hk, y + h) controlPoint2:NSMakePoint(x, y + h - hk)];
  [p closePath];
  return p;
}

/// The same outline with the low-y edge left open: the seam where a control and
/// the list below it meet must not be drawn twice, or it reads as a divider.
static NSBezierPath *perch_capsule_open(NSRect r, CGFloat highR) {
  CGFloat x = NSMinX(r), y = NSMinY(r), w = NSWidth(r), h = NSHeight(r);
  CGFloat k = highR * 0.55;
  NSBezierPath *p = [NSBezierPath bezierPath];
  [p moveToPoint:NSMakePoint(x, y)];
  [p lineToPoint:NSMakePoint(x, y + h - highR)];
  [p curveToPoint:NSMakePoint(x + highR, y + h) controlPoint1:NSMakePoint(x, y + h - k) controlPoint2:NSMakePoint(x + k, y + h)];
  [p lineToPoint:NSMakePoint(x + w - highR, y + h)];
  [p curveToPoint:NSMakePoint(x + w, y + h - highR) controlPoint1:NSMakePoint(x + w - k, y + h) controlPoint2:NSMakePoint(x + w, y + h - k)];
  [p lineToPoint:NSMakePoint(x + w, y)];
  return p;
}

@interface PerchFieldBox : NSView @end
@implementation PerchFieldBox
- (void)drawRect:(NSRect)dirty {
  NSBezierPath *shape = [NSBezierPath bezierPathWithRoundedRect:NSInsetRect(self.bounds, 0.5, 0.5) xRadius:14 yRadius:14];
  // Slightly brighter than the bubble wash so text areas lift off the blur.
  [[perch_color(0xfafbf6) colorWithAlphaComponent:0.5] setFill];
  [shape fill];
  [[perch_color(0xd7ddce) colorWithAlphaComponent:0.75] setStroke];
  [shape stroke];
}
@end

@interface PerchSelect : NSControl
@property (nonatomic, copy) NSArray<NSString *> *titles;
@property (nonatomic) NSInteger index;
/// Chevron rotation, 0 pointing down and 1 flipped up. Animated.
@property (nonatomic) CGFloat open;
/// Set the instant the list appears or goes, so the shared edge squares off
/// and un-squares with the list rather than trailing the chevron's easing.
@property (nonatomic) BOOL attached;
- (void)addItemsWithTitles:(NSArray<NSString *> *)titles;
- (NSString *)titleOfSelectedItem;
- (NSInteger)indexOfSelectedItem;
- (void)animateOpenTo:(CGFloat)open;
@end
@interface PerchScrim : NSView @end
@interface PerchRowList : NSView
@property (nonatomic, weak) PerchSelect *select;
@property (nonatomic, copy) NSArray<NSString *> *titles;
@property (nonatomic) NSInteger selected;
@end
static void perch_close_dropdown(void);
static PerchScrim *perch_scrim;
static PerchRowList *perch_list;
static void perch_close_dropdown(void) {
  if (!perch_list) return;
  PerchSelect *select = perch_list.select;
  select.attached = NO;
  [select animateOpenTo:0];
  PerchRowList *list = perch_list;
  [perch_scrim removeFromSuperview];
  perch_scrim = nil;
  perch_list = nil;
  // Rows ease back into the control instead of blinking away.
  CGFloat start = list.alphaValue;
  perch_animate(list, 0.16, ^(CGFloat t) { list.alphaValue = start * (1 - t); },
                ^{ [list removeFromSuperview]; });
}
@implementation PerchScrim
- (void)mouseDown:(NSEvent *)event { perch_close_dropdown(); }
@end
@implementation PerchRowList {
  NSInteger _hover;
}
- (instancetype)initWithFrame:(NSRect)frame {
  self = [super initWithFrame:frame];
  if (!self) return nil;
  _hover = -1;
  [self addTrackingArea:[[NSTrackingArea alloc] initWithRect:self.bounds
                                                     options:NSTrackingMouseMoved | NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways
                                                       owner:self userInfo:nil]];
  return self;
}
- (BOOL)isFlipped { return YES; }
- (void)drawRect:(NSRect)dirty {
  // Flipped, so minY is the top: square there to meet the select's own squared
  // bottom, and leave that edge unstroked so the join reads as one surface.
  NSRect box = NSInsetRect(self.bounds, 0.5, 0.5);
  [[perch_color(0xfffef9) colorWithAlphaComponent:0.97] setFill];
  [perch_capsule(box, 0, 14) fill];
  [[perch_color(0xdfe2d6) colorWithAlphaComponent:0.9] setStroke];
  [perch_capsule_open(box, 14) stroke];
  for (NSInteger i = 0; i < (NSInteger)self.titles.count; i++) {
    NSRect row = NSMakeRect(3, 3 + i * 26, self.bounds.size.width - 6, 26);
    if (i == _hover) {
      [perch_color(0xedf0e5) setFill];
      [[NSBezierPath bezierPathWithRoundedRect:NSInsetRect(row, 0, 1) xRadius:9 yRadius:9] fill];
    }
    BOOL on = i == self.selected;
    if (on) {
      [perch_color(0x7d9661) setFill];
      [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(12, row.origin.y + 11, 4, 4)] fill];
    }
    NSDictionary *attrs = @{
      NSFontAttributeName: [NSFont systemFontOfSize:12 weight:on ? NSFontWeightMedium : NSFontWeightRegular],
      NSForegroundColorAttributeName: perch_color(on ? 0x3d563b : 0x4d5949),
    };
    NSString *title = self.titles[i];
    [title drawAtPoint:NSMakePoint(22, row.origin.y + (26 - [title sizeWithAttributes:attrs].height) / 2) withAttributes:attrs];
  }
}
- (void)mouseMoved:(NSEvent *)event {
  NSInteger row = (NSInteger)([self convertPoint:event.locationInWindow fromView:nil].y / 26);
  if (row != _hover) {
    _hover = row;
    [self setNeedsDisplay:YES];
  }
}
- (void)mouseExited:(NSEvent *)event {
  _hover = -1;
  [self setNeedsDisplay:YES];
}
- (void)mouseDown:(NSEvent *)event {
  NSInteger row = (NSInteger)([self convertPoint:event.locationInWindow fromView:nil].y / 26);
  if (row >= 0 && row < (NSInteger)self.titles.count) {
    self.select.index = row;
    [self.select setNeedsDisplay:YES];
    if (self.select.action) [NSApp sendAction:self.select.action to:self.select.target from:self.select];
  }
  perch_close_dropdown();
}
@end
@implementation PerchSelect
- (void)animateOpenTo:(CGFloat)target {
  CGFloat from = _open;
  perch_animate(self, 0.22, ^(CGFloat t) { self.open = from + (target - from) * t; }, nil);
}
- (void)addItemsWithTitles:(NSArray<NSString *> *)titles {
  self.titles = titles;
  self.index = 0;
  [self setAccessibilityRole:NSAccessibilityPopUpButtonRole];
  [self setAccessibilityLabel:@"Action"];
}
- (NSString *)titleOfSelectedItem { return self.titles.count ? self.titles[self.index] : @""; }
- (NSInteger)indexOfSelectedItem { return self.index; }
- (void)setOpen:(CGFloat)open {
  _open = open;
  [self setNeedsDisplay:YES];
}
- (void)drawRect:(NSRect)dirty {
  CGFloat w = self.bounds.size.width, h = self.bounds.size.height;
  CGFloat radius = h / 2;
  NSRect box = NSInsetRect(self.bounds, 0.5, 0.5);
  [[perch_color(0xfafbf6) colorWithAlphaComponent:0.62] setFill];
  [perch_capsule(box, self.attached ? 0 : radius, radius) fill];
  [[perch_color(0xd7ddce) colorWithAlphaComponent:0.85] setStroke];
  if (self.attached) {
    [perch_capsule_open(box, radius) stroke];
  } else {
    [perch_capsule(box, radius, radius) stroke];
  }
  NSDictionary *attrs = @{
    NSFontAttributeName: [NSFont systemFontOfSize:12],
    NSForegroundColorAttributeName: perch_color(0x303d2b),
  };
  [self.titleOfSelectedItem drawAtPoint:NSMakePoint(14, (h - [self.titleOfSelectedItem sizeWithAttributes:attrs].height) / 2)
                         withAttributes:attrs];
  // Points down when closed and rotates up as the list drops out.
  CGFloat cx = w - 15, cy = h / 2, angle = M_PI * _open;
  CGFloat c = cos(angle), s = sin(angle);
  NSPoint arm[3] = {{-4.5, 2.2}, {0, -2.6}, {4.5, 2.2}};
  NSBezierPath *chevron = [NSBezierPath bezierPath];
  for (int i = 0; i < 3; i++) {
    NSPoint p = NSMakePoint(cx + arm[i].x * c - arm[i].y * s, cy + arm[i].x * s + arm[i].y * c);
    if (i == 0) [chevron moveToPoint:p]; else [chevron lineToPoint:p];
  }
  [perch_color(0x737c6d) setStroke];
  chevron.lineWidth = 1.8;
  chevron.lineCapStyle = NSLineCapStyleRound;
  chevron.lineJoinStyle = NSLineJoinStyleRound;
  [chevron stroke];
}
- (void)mouseDown:(NSEvent *)event {
  if (self.hidden) return;
  if (perch_list) {
    perch_close_dropdown();
    return;
  }
  NSView *root = self.window.contentView;
  CGFloat height = self.titles.count * 26 + 6;
  // Flush against the control's bottom edge: the two share a border.
  NSRect frame = NSMakeRect(self.frame.origin.x, self.frame.origin.y - height, self.frame.size.width, height);
  if (frame.origin.y < 0) frame.origin.y = 0;
  self.attached = YES;
  [self animateOpenTo:1];
  perch_scrim = [[PerchScrim alloc] initWithFrame:root.bounds];
  perch_list = [[PerchRowList alloc] initWithFrame:frame];
  perch_list.select = self;
  perch_list.titles = self.titles;
  perch_list.selected = self.index;
  perch_list.wantsLayer = YES;
  perch_list.layer.masksToBounds = NO;
  perch_list.layer.shadowColor = NSColor.blackColor.CGColor;
  perch_list.layer.shadowOpacity = 0.16;
  perch_list.layer.shadowRadius = 12;
  perch_list.layer.shadowOffset = CGSizeMake(0, -2);
  [root addSubview:perch_scrim];
  [root addSubview:perch_list];
  // Rows fade in; they sit at their final hit targets the whole time.
  perch_list.alphaValue = 0;
  perch_animate(perch_list, 0.16, ^(CGFloat t) { perch_list.alphaValue = t; }, nil);
}
@end

/// A quiet text link. Model switching is secondary to asking, so it earns a
/// hyperlink, not another pill competing with Run action and Connections.
@interface PerchLinkButton : NSControl <PerchHovering>
@property (nonatomic, copy) NSString *title;
@property (nonatomic) CGFloat hover;
@end
@implementation PerchLinkButton
- (instancetype)initWithFrame:(NSRect)frame title:(NSString *)title target:(id)target action:(SEL)action {
  self = [super initWithFrame:frame];
  if (!self) return nil;
  _title = [title copy];
  self.target = target;
  self.action = action;
  [self setAccessibilityRole:NSAccessibilityButtonRole];
  [self setAccessibilityLabel:title];
  [self addTrackingArea:[[NSTrackingArea alloc] initWithRect:self.bounds options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways | NSTrackingInVisibleRect owner:self userInfo:nil]];
  return self;
}
- (void)setTitle:(NSString *)title {
  _title = [title copy];
  [self setAccessibilityLabel:title];
  [self setNeedsDisplay:YES];
}
- (void)setHover:(CGFloat)hover { _hover = hover; [self setNeedsDisplay:YES]; }
- (void)drawRect:(NSRect)dirty {
  CGFloat alpha = self.isEnabled ? 1 : 0.55;
  NSDictionary *attrs = @{
    NSFontAttributeName: [NSFont systemFontOfSize:11 weight:NSFontWeightMedium],
    NSForegroundColorAttributeName: [perch_color(0x57704a) colorWithAlphaComponent:alpha],
  };
  NSMutableAttributedString *text = [[NSMutableAttributedString alloc] initWithString:self.title attributes:attrs];
  // Underline only on hover: the colour alone already reads as a link, and a
  // permanent underline makes the row noisier than the question it serves.
  if (_hover > 0.01) {
    [text addAttribute:NSUnderlineStyleAttributeName value:@(NSUnderlineStyleSingle) range:NSMakeRange(0, text.length)];
  }
  CGFloat textH = [self.title sizeWithAttributes:attrs].height;
  [text drawAtPoint:NSMakePoint(2, (NSHeight(self.bounds) - textH) / 2)];
}
- (void)mouseDown:(NSEvent *)event {
  if (self.isEnabled && self.action) [NSApp sendAction:self.action to:self.target from:self];
}
@end

/// An empty box with a lone caret gives no clue what to type; the prompt says
/// what this field is for until the first keystroke.
@interface PerchTextView : NSTextView
@property (nonatomic, copy) NSString *placeholder;
@end
@implementation PerchTextView
- (void)drawRect:(NSRect)rect {
  [super drawRect:rect];
  if (self.string.length || !self.placeholder.length) return;
  NSDictionary *attrs = @{
    NSFontAttributeName: self.font ?: [NSFont systemFontOfSize:12.5],
    NSForegroundColorAttributeName: [perch_color(0x6c7467) colorWithAlphaComponent:0.75],
  };
  [self.placeholder drawAtPoint:NSMakePoint(self.textContainerInset.width + self.textContainer.lineFragmentPadding,
                                            self.textContainerInset.height)
                 withAttributes:attrs];
}
- (void)didChangeText {
  [super didChangeText];
  [self setNeedsDisplay:YES];
}
@end

/// Borderless glyph button: a filled pill here would compete with the actions
/// that matter, so it is only a mark until the pointer finds it.
@interface PerchIconButton : NSControl <PerchHovering>
@property (nonatomic) CGFloat hover;
@end
@implementation PerchIconButton
- (instancetype)initWithFrame:(NSRect)frame target:(id)target action:(SEL)action {
  self = [super initWithFrame:frame];
  if (!self) return nil;
  self.target = target;
  self.action = action;
  [self setAccessibilityRole:NSAccessibilityButtonRole];
  [self setAccessibilityLabel:@"Close"];
  [self addTrackingArea:[[NSTrackingArea alloc] initWithRect:self.bounds options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways | NSTrackingInVisibleRect owner:self userInfo:nil]];
  return self;
}
- (void)setHover:(CGFloat)hover { _hover = hover; [self setNeedsDisplay:YES]; }
- (void)drawRect:(NSRect)dirty {
  CGFloat w = NSWidth(self.bounds), h = NSHeight(self.bounds), r = MIN(w, h) / 2;
  if (_hover > 0.01) {
    [[perch_color(0x5c6553) colorWithAlphaComponent:0.12 * _hover] setFill];
    [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(w / 2 - r, h / 2 - r, r * 2, r * 2)] fill];
  }
  CGFloat arm = 4;
  NSBezierPath *mark = [NSBezierPath bezierPath];
  [mark moveToPoint:NSMakePoint(w / 2 - arm, h / 2 - arm)];
  [mark lineToPoint:NSMakePoint(w / 2 + arm, h / 2 + arm)];
  [mark moveToPoint:NSMakePoint(w / 2 + arm, h / 2 - arm)];
  [mark lineToPoint:NSMakePoint(w / 2 - arm, h / 2 + arm)];
  mark.lineWidth = 1.6;
  mark.lineCapStyle = NSLineCapStyleRound;
  [[perch_color(0x5c6553) blendedColorWithFraction:_hover ofColor:perch_color(0x2f3a2b)] setStroke];
  [mark stroke];
}
- (void)mouseEntered:(NSEvent *)event { perch_hover_to(self, 1); }
- (void)mouseExited:(NSEvent *)event { perch_hover_to(self, 0); }
- (void)mouseDown:(NSEvent *)event {
  if (self.action) [NSApp sendAction:self.action to:self.target from:self];
}
@end

/// The whole folder line is the control, so what it does is legible from the
/// row itself rather than from an unlabelled ellipsis parked beside it.
@interface PerchFolderButton : NSControl <PerchHovering>
@property (nonatomic, copy) NSString *path;
@property (nonatomic) CGFloat hover;
@end
@implementation PerchFolderButton
- (instancetype)initWithFrame:(NSRect)frame target:(id)target action:(SEL)action {
  self = [super initWithFrame:frame];
  if (!self) return nil;
  self.target = target;
  self.action = action;
  [self setAccessibilityRole:NSAccessibilityButtonRole];
  [self addTrackingArea:[[NSTrackingArea alloc] initWithRect:self.bounds
                                                    options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways | NSTrackingInVisibleRect
                                                      owner:self userInfo:nil]];
  return self;
}
- (void)setHover:(CGFloat)hover { _hover = hover; [self setNeedsDisplay:YES]; }
- (void)setPath:(NSString *)path {
  _path = [path copy];
  self.toolTip = path.length ? path : nil;
  [self setAccessibilityLabel:path.length ? [NSString stringWithFormat:@"Working folder %@. Choose another.", path]
                                          : @"Choose a working folder"];
  [self setNeedsDisplay:YES];
}
- (void)drawRect:(NSRect)dirty {
  CGFloat w = NSWidth(self.bounds), h = NSHeight(self.bounds);
  NSRect box = NSInsetRect(self.bounds, 0.5, 0.5);
  NSBezierPath *shape = [NSBezierPath bezierPathWithRoundedRect:box xRadius:11 yRadius:11];
  [[perch_color(0xfafbf6) colorWithAlphaComponent:0.42 + 0.28 * _hover] setFill];
  [shape fill];
  [[perch_color(0xd7ddce) colorWithAlphaComponent:0.7 + 0.25 * _hover] setStroke];
  [shape stroke];

  // Folder mark: a body with a tab, filled rather than outlined so it still
  // reads as a folder at this size.
  CGFloat gh = 12, gw = 15, gx = 13, gy = (h - gh) / 2;
  CGFloat tab = gh * 0.26, body = gh - tab;
  NSBezierPath *folder = [NSBezierPath bezierPathWithRoundedRect:NSMakeRect(gx, gy, gw, body) xRadius:2 yRadius:2];
  [folder appendBezierPath:[NSBezierPath bezierPathWithRoundedRect:NSMakeRect(gx, gy + body - 2, gw * 0.5, tab + 2) xRadius:1.5 yRadius:1.5]];
  [[perch_color(0x7d9661) colorWithAlphaComponent:0.9] setFill];
  [folder fill];

  NSString *action = _path.length ? @"Change" : @"Choose";
  NSDictionary *actionAttrs = @{
    NSFontAttributeName: [NSFont systemFontOfSize:11.5 weight:NSFontWeightMedium],
    NSForegroundColorAttributeName: perch_color(0x57704a),
  };
  NSSize actionSize = [action sizeWithAttributes:actionAttrs];
  [action drawAtPoint:NSMakePoint(w - 14 - actionSize.width, (h - actionSize.height) / 2) withAttributes:actionAttrs];

  NSMutableParagraphStyle *style = [NSMutableParagraphStyle new];
  style.lineBreakMode = _path.length ? NSLineBreakByTruncatingMiddle : NSLineBreakByTruncatingTail;
  NSDictionary *attrs = @{
    NSFontAttributeName: [NSFont systemFontOfSize:12],
    NSForegroundColorAttributeName: _path.length ? perch_color(0x37432f) : perch_color(0x5c6553),
    NSParagraphStyleAttributeName: style,
  };
  NSString *label = _path.length ? _path.stringByAbbreviatingWithTildeInPath : @"Choose a folder, or drop me into one";
  CGFloat textX = gx + gw + 9;
  CGFloat available = w - 14 - actionSize.width - 10 - textX;
  CGFloat lineH = [label sizeWithAttributes:attrs].height;
  [label drawInRect:NSMakeRect(textX, (h - lineH) / 2, MAX(available, 10), lineH) withAttributes:attrs];
}
- (void)mouseEntered:(NSEvent *)event { perch_hover_to(self, 1); }
- (void)mouseExited:(NSEvent *)event { perch_hover_to(self, 0); }
- (void)mouseDown:(NSEvent *)event {
  if (self.action) [NSApp sendAction:self.action to:self.target from:self];
}
@end

/// The bubble's one fixed dimension; its height follows what is on show.
static const CGFloat PERCH_BUBBLE_W = 370;

/// Keep a scrolling text view filling the box it lives in as that box resizes.
static void perch_fit_text(NSScrollView *scroll, NSView *box, NSTextView *text) {
  scroll.frame = NSInsetRect(box.bounds, 1, 1);
  NSSize visible = scroll.contentSize;
  text.minSize = NSMakeSize(0, visible.height);
  text.maxSize = NSMakeSize(FLT_MAX, FLT_MAX);
  text.verticallyResizable = YES;
  text.horizontallyResizable = NO;
  text.autoresizingMask = NSViewWidthSizable;
  text.frame = NSMakeRect(0, 0, visible.width, MAX(visible.height, NSHeight(text.frame)));
  text.textContainer.containerSize = NSMakeSize(visible.width - text.textContainerInset.width * 2, FLT_MAX);
  text.textContainer.widthTracksTextView = YES;
}

@interface PerchBubble : NSObject <NSWindowDelegate, NSTextViewDelegate>
@property PerchBubblePanel *panel;
@property NSTextField *heading;
@property PerchTextView *prompt;
@property NSTextView *output;
@property PerchSegmented *mode;
@property PerchSelect *action;
@property PerchCheck *cloud;
@property PerchCheck *web;
@property PerchButton *approve;
@property PerchButton *reject;
@property PerchButton *send;
@property PerchButton *cancel;
@property PerchIconButton *closeButton;
@property PerchFolderButton *folderRow;
@property PerchLinkButton *modelLink;
@property PerchSelect *modelSelect;
@property PerchButton *connections;
@property PerchFieldBox *inputBox;
@property PerchFieldBox *outputBox;
@property NSScrollView *promptScroll;
@property NSScrollView *outputScroll;
@property NSNumber *pet;
@property NSNumber *task;
@property NSNumber *approval;
@property NSNumber *parent;
@property NSString *folder;
@property NSString *runtime;
@property BOOL remote;
@property NSTextField *hint;
- (void)dismiss;
- (void)syncActions;
- (void)reflow;
- (void)reflowAnimated:(BOOL)animated;
- (void)syncOutputVisibility;
@end
static PerchBubble *bubble;
@implementation PerchBubble
- (instancetype)init {
  if (!(self = [super init])) return nil;
  _panel = [[PerchBubblePanel alloc] initWithContentRect:NSMakeRect(0, 0, PERCH_BUBBLE_W, 320)
                                               styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                                                 backing:NSBackingStoreBuffered defer:NO];
  _panel.opaque = NO;
  _panel.backgroundColor = NSColor.clearColor;
  _panel.hasShadow = YES;
  // The palette is the web companion's light one, so the vibrancy behind it
  // has to stay light even when the system is in dark mode.
  _panel.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
  _panel.level = NSFloatingWindowLevel;
  _panel.hidesOnDeactivate = NO;
  _panel.becomesKeyOnlyIfNeeded = NO;
  _panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
  _panel.delegate = self;
  _panel.releasedWhenClosed = NO;
  _panel.contentView = [[PerchBubbleView alloc] initWithFrame:NSMakeRect(0, 0, PERCH_BUBBLE_W, 320)];
  NSView *view = _panel.contentView;
  _heading = [NSTextField labelWithString:@"A little help?"];
  _heading.font = [NSFont systemFontOfSize:16 weight:NSFontWeightSemibold];
  _heading.textColor = perch_color(0x354734);
  [view addSubview:_heading];
  _closeButton = [[PerchIconButton alloc] initWithFrame:NSZeroRect target:self action:@selector(close:)];
  [view addSubview:_closeButton];
  _folderRow = [[PerchFolderButton alloc] initWithFrame:NSZeroRect target:self action:@selector(choose:)];
  [view addSubview:_folderRow];
  // Two choices, because there are only two: run a built-in action, or ask the
  // model. Whether asking means a completion loop or an agent loop follows from
  // the connection, so it is not a question worth putting to the user.
  _mode = [[PerchSegmented alloc] initWithFrame:NSZeroRect labels:@[@"Actions", @"Ask"] target:self action:@selector(modeChanged:)];
  [view addSubview:_mode];
  _inputBox = [[PerchFieldBox alloc] initWithFrame:NSZeroRect];
  [view addSubview:_inputBox];
  _promptScroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
  _promptScroll.hasVerticalScroller = YES;
  _promptScroll.borderType = NSNoBorder;
  _promptScroll.drawsBackground = NO;
  _promptScroll.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  _prompt = [[PerchTextView alloc] initWithFrame:NSZeroRect];
  _prompt.font = [NSFont systemFontOfSize:12.5];
  _prompt.textColor = perch_color(0x303d2b);
  _prompt.richText = NO;
  _prompt.drawsBackground = NO;
  _prompt.insertionPointColor = perch_color(0x57704a);
  _prompt.placeholder = @"Ask for something…";
  _prompt.textContainerInset = NSMakeSize(10, 10);
  // Smart quotes and dashes corrupt paths and code typed into a prompt.
  _prompt.automaticQuoteSubstitutionEnabled = NO;
  _prompt.automaticDashSubstitutionEnabled = NO;
  _prompt.automaticTextReplacementEnabled = NO;
  // A missing question or description disables Send; typing has to re-enable it.
  _prompt.delegate = self;
  _promptScroll.documentView = _prompt;
  [_inputBox addSubview:_promptScroll];
  _cloud = [[PerchCheck alloc] initWithFrame:NSZeroRect title:@"Allow selected content to reach my AI provider" target:nil action:nil];
  [view addSubview:_cloud];
  _web = [[PerchCheck alloc] initWithFrame:NSZeroRect title:@"Allow web research" target:nil action:nil];
  [view addSubview:_web];
  // One quiet line to switch this pet's model; the picker behind it is
  // hidden until the click, and its list arrives asynchronously.
  _modelLink = [[PerchLinkButton alloc] initWithFrame:NSZeroRect title:@"Select another model" target:self action:@selector(pickModel:)];
  [view addSubview:_modelLink];
  _modelSelect = [[PerchSelect alloc] initWithFrame:NSZeroRect];
  _modelSelect.target = self;
  _modelSelect.action = @selector(chooseModel:);
  _modelSelect.hidden = YES;
  [view addSubview:_modelSelect];
  // The select sits above the prompt in z-order so its list can overlap it.
  _action = [[PerchSelect alloc] initWithFrame:NSZeroRect];
  [_action addItemsWithTitles:@[@"Sort by type", @"Sort by month", @"Rename with prefix", @"Find files", @"Find duplicates", @"My instructions"]];
  [view addSubview:_action];
  _send = [[PerchButton alloc] initWithFrame:NSZeroRect title:@"Run action" primary:YES target:self action:@selector(start:)];
  [view addSubview:_send];
  _connections = [[PerchButton alloc] initWithFrame:NSZeroRect title:@"Connections" primary:NO target:self action:@selector(settings:)];
  [view addSubview:_connections];
  _outputBox = [[PerchFieldBox alloc] initWithFrame:NSZeroRect];
  [view addSubview:_outputBox];
  _outputScroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
  _outputScroll.hasVerticalScroller = YES;
  _outputScroll.borderType = NSNoBorder;
  _outputScroll.drawsBackground = NO;
  _outputScroll.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  _output = [[NSTextView alloc] initWithFrame:NSZeroRect];
  _output.editable = NO;
  _output.richText = NO;
  _output.drawsBackground = NO;
  _output.font = [NSFont systemFontOfSize:12];
  _output.textColor = perch_color(0x37432f);
  _output.textContainerInset = NSMakeSize(10, 9);
  _output.usesFindBar = NO;
  _output.automaticLinkDetectionEnabled = NO;
  _output.automaticTextReplacementEnabled = NO;
  _outputScroll.documentView = _output;
  [_outputBox addSubview:_outputScroll];
  _approve = [[PerchButton alloc] initWithFrame:NSZeroRect title:@"Approve" primary:YES target:self action:@selector(accept:)];
  [view addSubview:_approve];
  _reject = [[PerchButton alloc] initWithFrame:NSZeroRect title:@"Reject" primary:NO target:self action:@selector(reject:)];
  [view addSubview:_reject];
  _cancel = [[PerchButton alloc] initWithFrame:NSZeroRect title:@"Cancel task" primary:NO target:self action:@selector(cancel:)];
  [view addSubview:_cancel];
  _approve.hidden = YES;
  _reject.hidden = YES;
  _cancel.hidden = YES;
  _hint = [NSTextField labelWithString:@""];
  _hint.font = [NSFont systemFontOfSize:11.5];
  _hint.textColor = perch_color(0x8a6d2f);
  _hint.lineBreakMode = NSLineBreakByTruncatingTail;
  [view addSubview:_hint];
  _outputBox.hidden = YES;
  [self applyMode];
  [self reflow];
  return self;
}
/// What the current mode still needs before it could possibly run, or nil.
/// Saying so here beats letting the task start and fail a second later. At
/// most one thing is ever missing, so the user is never handed a list of
/// conditions.
- (NSString *)blocker {
  BOOL ask = _mode.selectedSegment != 0;
  BOOL connected = _runtime.length && ![_runtime isEqual:@"none"];
  BOOL agent = [_runtime isEqual:@"codex"] || [_runtime isEqual:@"claude"];
  if (ask) {
    if (!connected) return @"Connect a model in Connections to ask.";
    if (_prompt.string.length == 0) return @"Type a question first.";
    return nil;
  }
  BOOL custom = _action.indexOfSelectedItem >= (NSInteger)_action.titles.count - 1;
  if (!connected) {
    // Built-in actions run offline, so they need neither a model nor consent —
    // but they do need a folder, and free-form instructions need a model.
    if (custom) return @"Pick a built-in action, or connect a model in Connections.";
    if (!_folder.length) return @"Choose a folder for built-in actions.";
    return nil;
  }
  if (custom && _prompt.string.length == 0) {
    return @"Describe what to do, or pick a built-in action.";
  }
  // A remote API model reading folder content is the one case the checkbox
  // exists for; an agent's start approval is its consent.
  if (!agent && _remote && !_cloud.on) {
    return @"Tick the box to allow this folder's content to reach the model.";
  }
  return nil;
}
- (void)syncActions {
  NSString *blocker = [self blocker];
  BOOL hadHint = _hint.hidden;
  _send.enabled = blocker == nil;
  _hint.stringValue = blocker ?: @"";
  _hint.hidden = blocker == nil;
  if (hadHint != _hint.hidden) [self reflow];
}
- (void)textDidChange:(NSNotification *)note {
  [self syncActions];
}
- (void)applyMode {
  BOOL ask = _mode.selectedSegment != 0;
  BOOL connected = _runtime.length && ![_runtime isEqual:@"none"];
  BOOL agent = [_runtime isEqual:@"codex"] || [_runtime isEqual:@"claude"];
  _send.title = ask ? @"Send request" : @"Run action";
  // Ask reads nothing on disk: no folder row, no action menu. Web research is
  // an ask thing; cloud-content consent belongs to Actions over a remote API
  // model (a local model never leaves the machine, an agent consents at start).
  _folderRow.hidden = ask;
  _action.hidden = ask;
  _cloud.hidden = ask || !connected || agent || !_remote;
  _web.hidden = !ask;
  _modelLink.hidden = !connected || !_modelSelect.hidden;
  _prompt.placeholder = ask ? @"Ask anything…" : @"Add details (optional)…";
}
- (void)modeChanged:(id)sender {
  perch_close_dropdown();
  [self applyMode];
  [self syncActions];
  [self reflow];
}
- (void)syncOutputVisibility {
  // An empty console is just a hole in the layout; the panel shrinks instead.
  _outputBox.hidden = _output.string.length == 0;
}
/// Replies are prose and belong in the UI face; only structured detail, like
/// the JSON behind an approval, earns a monospaced grid.
- (void)showOutput:(NSString *)text monospaced:(BOOL)monospaced {
  _output.font = monospaced ? [NSFont fontWithName:@"Menlo" size:11] : [NSFont systemFontOfSize:12];
  _output.string = [text ?: @"" stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
  // A fresh reply arriving pre-selected reads as a highlighted block.
  [_output setSelectedRange:NSMakeRange(0, 0)];
  [self syncOutputVisibility];
}
/// Wrap the reply at the box's real width so a one-line note is a one-line
/// strip rather than a fixed slab of empty console.
- (CGFloat)outputHeight {
  CGFloat inner = PERCH_BUBBLE_W - 44 - 2 - _output.textContainerInset.width * 2
                - _output.textContainer.lineFragmentPadding * 2;
  NSRect used = [_output.string boundingRectWithSize:NSMakeSize(inner, FLT_MAX)
                                             options:NSStringDrawingUsesLineFragmentOrigin
                                          attributes:@{NSFontAttributeName: _output.font ?: [NSFont systemFontOfSize:12]}];
  return MIN(MAX(ceil(NSHeight(used)) + _output.textContainerInset.height * 2 + 4, 36), 168);
}
- (void)place:(NSView *)target frame:(NSRect)frame {
  target.frame = frame;
}

/// Height the current rows need, regardless of where the panel is now.
- (CGFloat)contentHeight {
  const CGFloat notch = 16, edge = 16;
  const CGFloat headH = 28, folderH = 34, modeH = 28, checkH = 20, buttonH = 32, promptH = 84;
  const CGFloat gapS = 8, gapM = 12, gapL = 16, hintH = 18;
  BOOL folder = !_folderRow.hidden;
  BOOL approvals = !(_approve.hidden && _reject.hidden && _cancel.hidden);
  BOOL output = !_outputBox.hidden;
  BOOL hint = !_hint.hidden;
  const CGFloat linkH = 18, modelH = 28;
  CGFloat content = headH + gapS + modeH + gapM + promptH + gapM + buttonH;
  if (folder) content += folderH + gapL;
  if (hint) content += 8 + hintH;
  if (!_cloud.hidden) content += gapM + checkH;
  if (!_web.hidden) content += 6 + checkH;
  if (!_modelLink.hidden) content += 6 + linkH;
  if (!_modelSelect.hidden) content += 6 + modelH;
  if (output) content += gapM + [self outputHeight];
  if (approvals) content += gapM + buttonH;
  return content + (notch + edge) * 2;
}

- (void)reflow { [self reflowAnimated:YES]; }

/// Eases the panel from its current height to the height its rows need, so a
/// mode switch or an arriving reply grows the bubble instead of snapping it.
/// A fresh open snaps (perch_bubble_show places at final geometry), and an
/// off-screen panel never animates into a size nobody sees.
- (void)reflowAnimated:(BOOL)animated {
  perch_stop(self);
  CGFloat target = [self contentHeight];
  CGFloat from = NSHeight(_panel.frame);
  if (!animated || perch_reduce_motion() || !_panel.isVisible || fabs(from - target) < 0.5) {
    [self layoutAtHeight:target];
    return;
  }
  perch_animate(self, 0.22, ^(CGFloat t) { [self layoutAtHeight:from + (target - from) * t]; }, nil);
}

/// Lays the panel out top-down at `height`, skipping whatever is hidden, and
/// sizes the window to match — so turning a row off closes the gap instead of
/// leaving a hole where it used to be.
- (void)layoutAtHeight:(CGFloat)height {
  const CGFloat pad = 22, notch = 16, edge = 16;
  const CGFloat headH = 28, folderH = 34, modeH = 28, checkH = 20, buttonH = 32;
  const CGFloat promptH = 84;
  const CGFloat gapS = 8, gapM = 12, gapL = 16;
  CGFloat width = PERCH_BUBBLE_W - pad * 2;
  BOOL approvals = !(_approve.hidden && _reject.hidden && _cancel.hidden);
  BOOL output = !_outputBox.hidden;
  BOOL hint = !_hint.hidden;
  CGFloat outputH = output ? [self outputHeight] : 0;
  const CGFloat hintH = 18;

  // Grow from whichever edge the tail is not on, so the tail stays on the pet.
  // The origin moves incrementally against the panel's current frame, so each
  // tween tick re-anchors rather than re-fighting a fresh position. Window and
  // children commit together: animating them separately repeatedly rebuilds
  // the blur mask and leaves controls chasing their layout.
  NSRect frame = _panel.frame;
  PerchBubbleView *shell = (PerchBubbleView *)_panel.contentView;
  CGFloat delta = height - NSHeight(frame);
  frame.size.height = height;
  if (shell.tailUp) frame.origin.y -= delta;
  if (!NSEqualRects(_panel.frame, frame)) {
    [_panel setFrame:frame display:NO];
    [_panel invalidateShadow];
  }

  CGFloat y = height - notch - edge;
  y -= headH;
  [self place:_heading frame:NSMakeRect(pad, y, width - 40, headH)];
  [self place:_closeButton frame:NSMakeRect(PERCH_BUBBLE_W - pad - 28, y, 28, 28)];
  if (!_folderRow.hidden) {
    y -= gapS + folderH;
    [self place:_folderRow frame:NSMakeRect(pad, y, width, folderH)];
    y -= gapL + modeH;
  } else {
    y -= gapS + modeH;
  }
  CGFloat modeW = [_mode naturalWidth];
  [self place:_mode frame:NSMakeRect(pad, y, modeW, modeH)];
  [self place:_action frame:NSMakeRect(pad + modeW + 12, y, width - modeW - 12, modeH)];
  if (hint) {
    y -= 8 + hintH;
    [self place:_hint frame:NSMakeRect(pad, y, width, hintH)];
  }
  y -= gapM + promptH;
  [self place:_inputBox frame:NSMakeRect(pad, y, width, promptH)];
  if (!_cloud.hidden) {
    y -= gapM + checkH;
    [self place:_cloud frame:NSMakeRect(pad, y, width, checkH)];
  }
  if (!_web.hidden) {
    y -= 6 + checkH;
    [self place:_web frame:NSMakeRect(pad, y, width, checkH)];
  }
  // The model link and its picker share one slot; only one is ever visible.
  if (!_modelLink.hidden) {
    y -= 6 + 18;
    [self place:_modelLink frame:NSMakeRect(pad, y, width, 18)];
  }
  if (!_modelSelect.hidden) {
    y -= 6 + 28;
    [self place:_modelSelect frame:NSMakeRect(pad, y, width, 28)];
  }
  y -= gapM + buttonH;
  [self place:_connections frame:NSMakeRect(pad, y, 116, buttonH)];
  [self place:_send frame:NSMakeRect(PERCH_BUBBLE_W - pad - 116, y, 116, buttonH)];
  if (output) {
    y -= gapM + outputH;
    [self place:_outputBox frame:NSMakeRect(pad, y, width, outputH)];
  }
  if (approvals) {
    y -= gapM + buttonH;
    CGFloat x = pad;
    for (PerchButton *button in @[_approve, _reject, _cancel]) {
      if (button.hidden) continue;
      CGFloat w = button == _cancel ? 116 : (button == _reject ? 92 : 104);
      [self place:button frame:NSMakeRect(x, y, w, buttonH)];
      x += w + 10;
    }
  }
  perch_fit_text(_promptScroll, _inputBox, _prompt);
  perch_fit_text(_outputScroll, _outputBox, _output);
}
- (void)close:(id)sender { [self dismiss]; }
- (void)windowDidResignKey:(NSNotification *)note {
  perch_close_dropdown();
  if (!_panel.attachedSheet) [self dismiss];
}
/// Fades the panel out. The completion that orders the panel out only runs
/// when the tween finishes — a right-click that re-opens during the fade
/// cancels it via perch_stop, so the panel is never ordered out from under a
/// fresh open.
- (void)dismiss {
  perch_close_dropdown();
  perch_stop(self);
  NSWindow *panel = _panel;
  perch_stop(panel);
  CGFloat start = panel.alphaValue;
  perch_animate(panel, 0.14, ^(CGFloat t) { panel.alphaValue = start * (1 - t); },
                ^{ [panel orderOut:nil]; panel.alphaValue = 1; });
}

- (void)choose:(id)sender {
  NSOpenPanel *picker = [NSOpenPanel openPanel];
  picker.canChooseDirectories = YES;
  picker.canChooseFiles = NO;
  picker.allowsMultipleSelection = NO;
  [picker beginSheetModalForWindow:_panel completionHandler:^(NSModalResponse response) {
    if (response == NSModalResponseOK) emit(@{@"type": @"bind", @"petId": self.pet, @"folder": picker.URL.path});
  }];
}
- (void)settings:(id)sender { emit(@{@"type": @"settings", @"petId": _pet}); [self dismiss]; }
- (void)pickModel:(id)sender {
  // Disabled while the list loads: a second click would queue a second fetch
  // behind the first for no gain. Any reply re-enables it.
  _modelLink.enabled = NO;
  emit(@{@"type": @"models", @"petId": _pet});
}
- (void)chooseModel:(id)sender {
  NSString *model = _modelSelect.titleOfSelectedItem;
  perch_close_dropdown();
  _modelSelect.hidden = YES;
  _modelLink.enabled = YES;
  _modelLink.hidden = !_runtime.length || [_runtime isEqual:@"none"];
  [self reflow];
  if (model.length) emit(@{@"type": @"model", @"petId": _pet, @"model": model});
}
- (void)start:(id)sender {
  // "My instructions" is the last dropdown entry and sends an empty preset;
  // the backend composes the prompt from the raw text and connection.
  NSArray *presets = @[@"sort_type", @"sort_date", @"rename", @"find", @"duplicates", @""];
  BOOL ask = _mode.selectedSegment != 0;
  emit(@{@"type": @"start", @"petId": _pet, @"prompt": _prompt.string ?: @"", @"preset": ask ? @"" : presets[_action.indexOfSelectedItem], @"mode": ask ? @"ask" : @"actions", @"allowCloud": ask ? @YES : @(_cloud.on), @"allowWeb": @(_web.on), @"parentId": _parent ?: NSNull.null});
}
- (void)accept:(id)sender {
  if (_task && !_approval) { emit(@{@"type": @"undo", @"id": _task}); return; }
  if (_task && _approval) emit(@{@"type": @"approve", @"id": _task, @"approvalId": _approval, @"accept": @YES});
  _approve.enabled = NO;
  _reject.enabled = NO;
}
- (void)reject:(id)sender {
  if (_task && _approval) emit(@{@"type": @"approve", @"id": _task, @"approvalId": _approval, @"accept": @NO});
  _approve.enabled = NO;
  _reject.enabled = NO;
}
- (void)cancel:(id)sender { if (_task) emit(@{@"type": @"cancel", @"id": _task}); }
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
void perch_bubble_show(const char *json) {
  NSDictionary *data = [NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
  perch_close_dropdown();
  if (!bubble) bubble = [PerchBubble new];
  if (![bubble.pet isEqual:data[@"petId"]]) {
    bubble.prompt.string = @"";
    bubble.output.string = @"";
    bubble.task = nil;
    bubble.parent = nil;
    bubble.approval = nil;
    bubble.approve.hidden = YES;
    bubble.reject.hidden = YES;
    bubble.cancel.hidden = YES;
  }
  bubble.pet = data[@"petId"];
  bubble.folder = data[@"folder"];
  bubble.heading.stringValue = [NSString stringWithFormat:@"Ask %@", data[@"name"] ?: @"your pet"];
  bubble.runtime = data[@"runtime"];
  bubble.remote = [data[@"remote"] boolValue];
  // A fresh open never starts mid-pick: the picker resets and the link is
  // armed again, whatever state the previous pet left it in.
  bubble.modelSelect.hidden = YES;
  bubble.modelLink.enabled = YES;
  bubble.folderRow.path = bubble.folder;
  // The connection may have changed since the bubble last closed, so which
  // rows a mode needs is recomputed here too.
  [bubble applyMode];
  [bubble syncActions];
  [bubble syncOutputVisibility];
  [bubble reflowAnimated:NO];
  NSPoint point = NSEvent.mouseLocation;
  NSScreen *screen = NSScreen.mainScreen;
  for (NSScreen *candidate in NSScreen.screens)
    if (NSPointInRect(point, candidate.frame)) { screen = candidate; break; }
  NSRect visible = screen.visibleFrame;
  CGFloat height = NSHeight(bubble.panel.frame);
  CGFloat x = MAX(NSMinX(visible) + 8, MIN(point.x - 48, NSMaxX(visible) - PERCH_BUBBLE_W - 8));
  BOOL below = NO;
  CGFloat y = point.y + 12;
  if (y + height > NSMaxY(visible)) { y = point.y - height - 8; below = YES; }
  y = MAX(NSMinY(visible) + 8, MIN(y, NSMaxY(visible) - height - 8));
  // The panel's x/y get clamped to stay on-screen, which can push it away from
  // the pet it belongs to; point the tail at the real anchor, not a fixed spot.
  PerchBubbleView *bubbleView = (PerchBubbleView *)bubble.panel.contentView;
  bubbleView.tailX = point.x - x;
  bubbleView.tailUp = below;

  [bubble.panel setFrame:NSMakeRect(x, y, PERCH_BUBBLE_W, height) display:NO];
  // A contextual surface opens at its final position, ready for input, and
  // fades in. perch_stop cancels a dismiss fade still in flight — its
  // completion never runs, so an interrupted close cannot order the panel out
  // from under this open.
  NSWindow *panel = bubble.panel;
  perch_stop(panel);
  CGFloat start = panel.isVisible ? panel.alphaValue : 0;
  [panel orderFrontRegardless];
  perch_animate(panel, 0.18, ^(CGFloat t) { panel.alphaValue = start + (1 - start) * t; }, nil);
  [panel makeKeyWindow];
  [panel makeFirstResponder:bubble.prompt];
}
void perch_bubble_update(const char *json) {
  if (!bubble) return;
  NSDictionary *data = [NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
  if (data[@"folder"]) {
    bubble.folder = data[@"folder"];
    bubble.folderRow.path = bubble.folder;
    [bubble syncActions];
  }
  if (data[@"models"]) {
    // The model list arrives asynchronously; only a list with entries is
    // worth swapping the link for the picker.
    if ([data[@"models"] isKindOfClass:NSArray.class] && [data[@"models"] count] > 0) {
      [bubble.modelSelect addItemsWithTitles:data[@"models"]];
      bubble.modelSelect.hidden = NO;
      [bubble applyMode];
      [bubble reflow];
      return;
    }
    bubble.modelLink.enabled = YES;
  }
  if (data[@"message"]) {
    bubble.modelLink.enabled = YES;
    [bubble showOutput:data[@"message"] monospaced:NO];
    [bubble reflow];
    return;
  }
  if (![bubble.pet isEqual:data[@"request"][@"petId"]]) return;
  bubble.task = data[@"id"];
  NSString *status = data[@"status"];
  BOOL done = [@[@"completed", @"cancelled", @"failed", @"interrupted", @"undone"] containsObject:status];
  bubble.send.enabled = done;
  bubble.cancel.hidden = done;
  if (done) bubble.parent = bubble.task;
  NSDictionary *approval = data[@"approval"];
  BOOL waiting = [status isEqual:@"waiting"] && [approval isKindOfClass:NSDictionary.class];
  bubble.approve.hidden = !waiting;
  bubble.reject.hidden = !waiting;
  bubble.approve.enabled = YES;
  bubble.reject.enabled = YES;
  bubble.approve.title = @"Approve";
  if (!waiting) bubble.approval = nil;
  if (done) {
    for (NSDictionary *op in data[@"operations"]) {
      if ([@[@"applied", @"applying"] containsObject:op[@"state"]]) { bubble.approve.hidden = NO; bubble.approve.title = @"Undo"; break; }
    }
  }
  if (waiting) {
    bubble.approval = approval[@"id"];
    NSData *details = [NSJSONSerialization dataWithJSONObject:approval[@"details"] options:NSJSONWritingPrettyPrinted error:nil];
    [bubble showOutput:[NSString stringWithFormat:@"%@\n%@", approval[@"title"], [[NSString alloc] initWithData:details encoding:NSUTF8StringEncoding]]
            monospaced:YES];
  } else {
    NSString *error = [data[@"error"] isKindOfClass:NSString.class] ? data[@"error"] : @"";
    [bubble showOutput:[NSString stringWithFormat:@"%@\n%@%@", data[@"progress"] ?: @"", error, data[@"result"] ?: @""]
            monospaced:NO];
  }
  [bubble reflow];
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