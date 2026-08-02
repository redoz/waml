# Navigate and Return

**Goal:** A reader follows a link and then returns.

**Why:** A bundle is a graph. To read a graph is to walk in it. A walk with no
return path stops the reader.

**Done when:** A click on a link in text or on a node in a diagram opens the
target. Back returns to the previous position, with the same scroll position
and the same selection. Forward moves to the position again.

**Status:** done — unverified
**MVP:** yes

## Notes

- Tree rows, breadcrumb segments, and links in text use one navigation policy.
  The policy covers documents, directories, and fragments in a document. Three
  call sites with three behaviors make the function impossible to test. One
  policy makes this goal possible to complete.
- To reveal is not to navigate. A click on a breadcrumb shows the position of
  the current document in the tree. It opens no document and it does not change
  a folder.
- The navigation history has a limit. A long session cannot increase it without
  end.
- A live view stays after a model revision if the revision is compatible with
  it. If the revision is not compatible, the tool removes the view and makes it
  again with the full lifecycle. The tool does not repair a view in part,
  because a partially repaired view shows incorrect content.
- Verify the accuracy of the position. Return to the correct document operates.
  Return to the correct position in that document is not verified.
