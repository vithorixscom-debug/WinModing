import { ref } from "vue";
import { version } from "../../package.json";

export const allowState = ref(false);
export const installState = ref(false);
export const updateState = ref(false);
export const latestBetaCommitTruncatedSha = ref("");
export const latestBetaCommitLink = ref("");
export const launcherUrl = "https://github.com/vithorixscom-debug/WinModing";

export async function getBranches() {}

export async function getRemote() {
  console.info(`WinModing ${version} — updater disabled in this build`);
}
