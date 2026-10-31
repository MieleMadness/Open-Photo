# Media viewer — prepared 0.56.23

File Details now uses a wider surface, a padded preview that follows window height, and full-width photo Edit & Export controls. Narrow windows stack fields without reducing the usable image to the previous fixed 220-pixel box. Image orientation/aspect ratio are retained, and exports still create new files through existing protections.

## Zoom and motion

- Wheel/trackpad over an image zooms near the pointer. Enlarged images can be dragged to pan; keyboard arrows pan when zoomed, otherwise normal media navigation remains available.
- **+ / −** zoom; **0 / Fit** resets the view. **100% preview** displays the bounded decoded preview at its native pixels. This is not a claim of original-file pixel resolution: existing 1920/2048-edge preview limits remain intact.
- Outward wheel scrolling at fit can continue through the details panel. Wheel events elsewhere keep their normal behavior. Pan is clamped when the stage changes size, and a new media identity resets zoom.
- A Live image starts as a still. **Play Live image** switches the same stage to its paired motion; **Show still image** returns to the image. Standalone videos and Live motion use a single video element with native controls. Extra transport, scrubber and video-frame filmstrip controls are removed from viewer mode; compatibility fallback and useful errors remain.
- Closing/switching media releases detached decoders. View transforms never alter crop coordinates, project state or output geometry.

## Fullscreen

The media occupies almost the whole window. Compact top controls remain accessible, and previous/next buttons sit at the side edges. The bottom **library filmstrip** follows the current Library's sort/filter order and includes images, Live pairs and videos. **Hide filmstrip / Show filmstrip** remembers the preference and returns the space to the image when hidden.

The filmstrip renders only a bounded horizontal thumbnail window, highlights the selected item and keeps it in view. It does not autoplay neighboring videos. Closing the viewer preserves the underlying Library scroll position. Live playback and matching-time video search use the same stage in either viewer size.

Escape closes the viewer; Left/Right browse when a text field or native player does not own the key. Focus remains visible, with named buttons for all actions. Local browser fixtures cover both themes, normal/small windows, padding, zoom/reset, fullscreen, Live playback and the indexing dashboard. Installed WebView2 interaction/listening remains a handoff check.
