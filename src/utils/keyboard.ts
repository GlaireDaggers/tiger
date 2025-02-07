import {
  acknowledgeError,
  beginExportAs,
  beginRenameSelection,
  browseSelection,
  browseToEnd,
  browseToStart,
  cancelExit,
  cancelExportAs,
  cancelRelocateFrames,
  centerWorkbench,
  closeAllDocuments,
  closeCurrentDocument,
  copy,
  cut,
  deleteSelection,
  doExport,
  newDocument,
  nudgeSelection,
  openDocuments,
  paste,
  pause,
  play,
  redo,
  resetTimelineZoom,
  resetWorkbenchZoom,
  save,
  saveAll,
  saveAs,
  selectAll,
  undo,
  zoomInTimeline,
  zoomInWorkbench,
  zoomOutTimeline,
  zoomOutWorkbench,
} from "@/backend/api";
import { BrowseDirection, NudgeDirection } from "@/backend/dto";
import { useStateStore } from "@/stores/state";

function onKeyDown(event: KeyboardEvent) {
  const isActiveElementKeyboardFriendly =
    (document.activeElement as HTMLInputElement).tabIndex != -1;

  if (document.activeElement?.tagName == "INPUT") {
    return;
  }

  const state = useStateStore();

  if (event.ctrlKey) {
    if (event.key == "n") {
      newDocument();
    } else if (event.key == "o") {
      openDocuments();
    } else if (event.key == "s") {
      if (event.altKey) {
        saveAll();
      } else {
        save();
      }
    } else if (event.key == "S") {
      event.preventDefault();
      saveAs(state.currentDocumentPath);
    } else if (event.key == "e") {
      doExport();
    } else if (event.key == "E") {
      event.preventDefault();
      beginExportAs();
    } else if (event.key == "w") {
      closeCurrentDocument();
    } else if (event.key == "W") {
      closeAllDocuments();
    } else if (event.key == "z") {
      event.preventDefault();
      undo();
    } else if (event.key == "Z") {
      event.preventDefault();
      redo();
    } else if (event.key == "x") {
      cut();
    } else if (event.key == "c") {
      copy();
    } else if (event.key == "v") {
      paste();
    } else if (event.key == " ") {
      centerWorkbench();
    } else if (event.key == "+" || event.key == "=") {
      if (event.altKey) {
        zoomInTimeline();
      } else {
        zoomInWorkbench();
      }
    } else if (event.key == "-") {
      if (event.altKey) {
        zoomOutTimeline();
      } else {
        zoomOutWorkbench();
      }
    } else if (event.key == "0") {
      if (event.altKey) {
        resetTimelineZoom();
      } else {
        resetWorkbenchZoom();
      }
    } else if (event.key == "a") {
      selectAll();
    } else if (event.key == "ArrowUp") {
      nudgeSelection(NudgeDirection.Up, event.shiftKey);
    } else if (event.key == "ArrowDown") {
      nudgeSelection(NudgeDirection.Down, event.shiftKey);
    } else if (event.key == "ArrowLeft") {
      nudgeSelection(NudgeDirection.Left, event.shiftKey);
    } else if (event.key == "ArrowRight") {
      nudgeSelection(NudgeDirection.Right, event.shiftKey);
    }
  } else {
    if (event.key == " ") {
      if (!isActiveElementKeyboardFriendly) {
        event.preventDefault();
        if (state.currentDocument?.timelineIsPlaying) {
          pause();
        } else {
          play();
        }
      }
    } else if (event.key == "Delete") {
      deleteSelection();
    } else if (event.key == "ArrowUp") {
      event.preventDefault();
      browseSelection(BrowseDirection.Up, event.shiftKey);
    } else if (event.key == "ArrowDown") {
      event.preventDefault();
      browseSelection(BrowseDirection.Down, event.shiftKey);
    } else if (event.key == "ArrowLeft") {
      event.preventDefault();
      browseSelection(BrowseDirection.Left, event.shiftKey);
    } else if (event.key == "ArrowRight") {
      event.preventDefault();
      browseSelection(BrowseDirection.Right, event.shiftKey);
    } else if (event.key == "Home") {
      browseToStart(event.shiftKey);
    } else if (event.key == "End") {
      browseToEnd(event.shiftKey);
    } else if (event.key == "F2") {
      beginRenameSelection();
    } else if (event.key == "Enter") {
      if (!isActiveElementKeyboardFriendly) {
        event.preventDefault();
      }
    } else if (event.key == "Escape") {
      if (state.error) {
        acknowledgeError();
      } else if (state.currentDocument?.wasCloseRequested) {
        cancelExit();
      } else if (state.currentDocument?.framesBeingRelocated) {
        cancelRelocateFrames();
      } else if (state.currentDocument?.exportSettingsBeingEdited) {
        cancelExportAs();
      }
    }
  }
}

export function registerKeyboardShortcuts() {
  window.addEventListener("keydown", onKeyDown);
}

export function unregisterKeyboardShortcuts() {
  window.removeEventListener("keydown", onKeyDown);
}
