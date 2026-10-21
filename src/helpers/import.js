export async function get_importable_instances() {
  return [];
}

export async function import_instance() {
  throw new Error("Import from other launchers is not available in this build.");
}

export async function is_valid_importable_instance() {
  return false;
}

export async function get_default_launcher_path() {
  return null;
}
