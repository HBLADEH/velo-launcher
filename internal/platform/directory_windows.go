package platform

// OpenDirectory opens a local directory in Explorer without shell interpolation.
func OpenDirectory(path string) error { return shellExecute(path, "", "") }
