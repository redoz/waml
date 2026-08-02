# Run in a Browser

**Goal:** The same editor operates as a web artifact and shows the same views
as the desktop form.

**Why:** There is one application with two delivery forms. A separate web
viewer is a second product to keep correct.

**Done when:** The web form draws each view that the native form draws. The web
form starts in a time that a reader accepts. A failure causes a message. The
canvas is not empty.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The build command must not use threads. The threaded build needs
  cross-origin isolation headers. The publication service cannot set headers.
- The start time was the primary defect and is now much better. A change to
  make the shader links in one batch decreased the start time from
  approximately nine seconds to less than two seconds.
- The web renderer has known differences from the native renderer. One
  renderer does not draw some diagonal content. Thus "the same views" is not
  fully true at this time.
- A verification script prevents a build that has no JavaScript glue code.
