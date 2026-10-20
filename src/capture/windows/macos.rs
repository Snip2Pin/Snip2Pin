//! Query CoreGraphics once, avoiding xcap's repeated full-list property lookups.
#![allow(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};

use objc2_core_foundation::{CFDictionary, CFNumber, CFString, CFType, CGRect};
use objc2_core_graphics::{
    CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption,
    kCGDockWindowLevel, kCGNormalWindowLevel, kCGWindowAlpha, kCGWindowBounds, kCGWindowLayer,
};

use super::WindowBounds;

fn value<'a>(dictionary: &'a CFDictionary, key: &CFString) -> Option<&'a CFType> {
    // SAFETY: The immutable system-owned window-info dictionary contains CFType
    // values. The retained list keeps the dictionary and values alive throughout
    // this borrow. A missing key returns None; callers check each concrete type.
    unsafe {
        dictionary
            .value((key as *const CFString).cast())
            .cast::<CFType>()
            .as_ref()
    }
}

fn describe(info: &CFDictionary) -> Option<WindowBounds> {
    // SAFETY: These framework string constants are immutable and always present
    // on the minimum supported macOS version.
    let (layer, alpha, bounds) = unsafe { (kCGWindowLayer, kCGWindowAlpha, kCGWindowBounds) };
    let layer = value(info, layer)?.downcast_ref::<CFNumber>()?.as_i32()?;
    let alpha = value(info, alpha)?.downcast_ref::<CFNumber>()?.as_f64()?;
    if !(kCGNormalWindowLevel..kCGDockWindowLevel).contains(&layer)
        || !alpha.is_finite()
        || alpha <= 0.0
    {
        return None;
    }
    let bounds = value(info, bounds)?.downcast_ref::<CFDictionary>()?;
    let mut rect = CGRect::default();
    // SAFETY: bounds is a live CoreGraphics bounds dictionary from the retained
    // window list, and rect is a valid exclusively borrowed output rectangle.
    if !unsafe { CGRectMakeWithDictionaryRepresentation(Some(bounds), &mut rect) } {
        return None;
    }
    Some(WindowBounds {
        origin: [rect.origin.x, rect.origin.y],
        size: [rect.size.width, rect.size.height],
    })
}

pub(in crate::capture) fn visible(cancelled: &AtomicBool) -> Vec<WindowBounds> {
    let Some(windows) = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        0,
    ) else {
        return Vec::new();
    };
    (0..windows.count())
        .take_while(|_| !cancelled.load(Ordering::Acquire))
        .filter_map(|index| {
            // SAFETY: CoreGraphics returns an immutable array of CFDictionary
            // objects in front-to-back order. The retained list owns each value.
            let value = unsafe { windows.value_at_index(index).cast::<CFType>().as_ref() }?;
            describe(value.downcast_ref::<CFDictionary>()?)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_core_foundation::{CGPoint, CGSize};
    use objc2_core_graphics::{
        CGRectCreateDictionaryRepresentation, kCGFloatingWindowLevel, kCGModalPanelWindowLevel,
        kCGUtilityWindowLevel,
    };

    #[test]
    fn window_metadata_keeps_normal_and_floating_windows_but_excludes_system_overlays() {
        let bounds = CGRectCreateDictionaryRepresentation(CGRect {
            origin: CGPoint::new(-300.0, 40.0),
            size: CGSize::new(200.0, 100.0),
        });
        // SAFETY: The framework constants are immutable CFStrings.
        let keys = unsafe { [kCGWindowLayer, kCGWindowAlpha, kCGWindowBounds] };
        for (level, alpha, accepted) in [
            (kCGNormalWindowLevel, 1.0, true),
            (kCGFloatingWindowLevel, 1.0, true),
            (kCGModalPanelWindowLevel, 1.0, true),
            (kCGUtilityWindowLevel, 1.0, true),
            (kCGDockWindowLevel, 1.0, false),
            (kCGNormalWindowLevel, 0.0, false),
            (kCGNormalWindowLevel, f64::NAN, false),
        ] {
            let layer = CFNumber::new_i32(level);
            let alpha = CFNumber::new_f64(alpha);
            let values: [&CFType; 3] = [&layer, &alpha, &bounds];
            let info = CFDictionary::from_slices(&keys, &values);
            let result = describe(info.as_ref());
            assert_eq!(result.is_some(), accepted);
            if let Some(window) = result {
                assert_eq!(window.origin, [-300.0, 40.0]);
                assert_eq!(window.size, [200.0, 100.0]);
            }
        }
    }
}
