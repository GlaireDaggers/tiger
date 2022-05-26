import { ContentTab } from "@/api/dto";
import { useAppStore } from "@/stores/app";
import { invoke } from "@tauri-apps/api";

export async function undo(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("undo"));
}

export async function redo(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("redo"));
}

export async function focusContentTab(contentTab: ContentTab): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("focus_content_tab", { contentTab: contentTab }));
}

export async function clearSelection(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("clear_selection"));
}

export async function selectFrame(
  path: string,
  shift: boolean,
  ctrl: boolean
): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(
    await invoke("select_frame", {
      path: path,
      shift: shift,
      ctrl: ctrl,
    })
  );
}

export async function selectAnimation(
  name: string,
  shift: boolean,
  ctrl: boolean
): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(
    await invoke("select_animation", {
      name: name,
      shift: shift,
      ctrl: ctrl,
    })
  );
}

export async function pan(delta: [number, number]): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("pan", { delta: delta }));
}

export async function centerWorkbench(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("center_workbench"));
}

export async function zoomInWorkbench(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("zoom_in_workbench"));
}

export async function zoomOutWorkbench(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("zoom_out_workbench"));
}

export async function resetWorkbenchZoom(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("reset_workbench_zoom"));
}

export async function editAnimation(name: string): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("edit_animation", { name: name }));
}

export async function renameAnimation(
  oldName: string,
  newName: string
): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(
    await invoke("rename_animation", {
      oldName: oldName,
      newName: newName,
    })
  );
}

export async function deleteAnimation(name: string): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("delete_animation", { name: name }));
}

export async function tick(deltaTimeMillis: number): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("tick", { deltaTimeMillis: deltaTimeMillis }));
}

export async function play(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("play"));
}

export async function pause(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("pause"));
}

export async function scrubTimeline(timeMillis: number): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("scrub_timeline", { timeMillis: timeMillis }));
}

export async function zoomInTimeline(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("zoom_in_timeline"));
}

export async function zoomOutTimeline(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("zoom_out_timeline"));
}

export async function resetTimelineZoom(): Promise<void> {
  const appStore = useAppStore();
  appStore.patch(await invoke("reset_timeline_zoom"));
}
