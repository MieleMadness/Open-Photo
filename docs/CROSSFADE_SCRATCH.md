# Experimental crossfade scratch-ownership gate

A test-only native helper now checks paths before a future FFmpeg crossfade run. It requires 2–200 distinct existing regular Matroska parts as direct children of a canonical, non-symlink temporary directory. The MP4 output must be a new direct child of the same directory, never an existing file or symlink. Paths outside scratch and duplicate inputs are rejected.

This **does not yet constitute safe production execution**. A later caller must create and own the temporary directory securely, prevent filesystem races between verification and FFmpeg open, validate source fingerprints and all normalized media streams, enforce cancellation and disk/output budgets, and verify decoded output before publication. It currently creates no executable path from the user-facing editor.

The test fixture exercises distinct inputs, duplicate parts, missing required count, wrong output extension, an already existing destination and an input outside scratch. Windows symlink/reparse-point and racing path replacement require dedicated acceptance before using this gate for actual exports.
