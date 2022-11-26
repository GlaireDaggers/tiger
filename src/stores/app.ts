import {
  Animation,
  AppState,
  Frame,
  Hitbox,
  Keyframe,
  Patch,
  Sequence,
} from "@/api/dto";
import { applyPatch } from "fast-json-patch";
import { defineStore, acceptHMRUpdate } from "pinia";

export const useAppStore = defineStore("app", {
  state: () =>
    ({
      documents: [],
      currentDocumentPath: null,
      recentDocumentPaths: [],
      clipboardManifest: null,
      isReleaseBuild: false,
      error: null,
    } as AppState),
  actions: {
    patch(patch: Patch) {
      applyPatch(this.$state, patch, false);
    },
  },
  getters: {
    currentDocument(state) {
      for (let document of state.documents) {
        if (document.path == state.currentDocumentPath) {
          return document;
        }
      }
      return null;
    },
    sortedAnimations(): Animation[] | null {
      if (!this.currentDocument?.sheet.animations) {
        return null;
      }
      let animations = Object.values(this.currentDocument.sheet.animations);
      animations.sort((a, b) => {
        const nameA = a.name.toLowerCase();
        const nameB = b.name.toLowerCase();
        if (nameA < nameB) {
          return -1;
        }
        if (nameA > nameB) {
          return 1;
        }
        return 0;
      });
      return animations;
    },
    currentAnimation(): Animation | null {
      if (this.currentDocument?.currentAnimationName) {
        return this.currentDocument.sheet.animations[
          this.currentDocument.currentAnimationName
        ];
      }
      return null;
    },
    currentSequence(): Sequence | null {
      if (
        this.currentAnimation &&
        this.currentDocument?.currentSequenceDirection
      ) {
        return this.currentAnimation.sequences[
          this.currentDocument.currentSequenceDirection
        ];
      }
      return null;
    },
    currentKeyframe(): Keyframe | null {
      if (
        this.currentSequence &&
        this.currentDocument?.currentKeyframeIndex != null
      ) {
        return this.currentSequence.keyframes[
          this.currentDocument?.currentKeyframeIndex
        ];
      }
      return null;
    },
    selectedFrames(): Frame[] | null {
      if (!this.currentDocument) {
        return null;
      }
      return this.currentDocument.sheet.frames.filter(
        (frame) => frame.selected
      );
    },
    selectedAnimations(): Animation[] | null {
      if (!this.sortedAnimations) {
        return null;
      }
      return this.sortedAnimations.filter((animation) => animation.selected);
    },
    selectedHitboxes(): Hitbox[] | null {
      if (!this.currentKeyframe) {
        return null;
      }
      return this.currentKeyframe.hitboxes.filter((hitbox) => hitbox.selected);
    },
    selectedKeyframes(): Keyframe[] | null {
      if (!this.currentAnimation) {
        return null;
      }
      return Object.values(this.currentAnimation.sequences).flatMap(
        (sequence) => sequence.keyframes.filter((keyframe) => keyframe.selected)
      );
    },
    canCut(): boolean {
      return (
        !!this.selectedAnimations?.length ||
        !!this.selectedKeyframes?.length ||
        !!this.selectedHitboxes?.length
      );
    },
    canCopy(): boolean {
      return (
        !!this.selectedFrames?.length ||
        !!this.selectedAnimations?.length ||
        !!this.selectedKeyframes?.length ||
        !!this.selectedHitboxes?.length
      );
    },
  },
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useAppStore, import.meta.hot));
}
