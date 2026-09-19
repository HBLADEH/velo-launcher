export namespace config {
	
	export class Search {
	    fuzzy: boolean;
	    history_weight: number;
	
	    static createFrom(source: any = {}) {
	        return new Search(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.fuzzy = source["fuzzy"];
	        this.history_weight = source["history_weight"];
	    }
	}
	export class Config {
	    version: number;
	    hotkey: string;
	    max_results: number;
	    theme: string;
	    launch_at_startup: boolean;
	    search: Search;
	    custom_directories: string[];
	    scan_program_files: boolean;
	    refresh_minutes: number;
	
	    static createFrom(source: any = {}) {
	        return new Config(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.version = source["version"];
	        this.hotkey = source["hotkey"];
	        this.max_results = source["max_results"];
	        this.theme = source["theme"];
	        this.launch_at_startup = source["launch_at_startup"];
	        this.search = this.convertValues(source["search"], Search);
	        this.custom_directories = source["custom_directories"];
	        this.scan_program_files = source["scan_program_files"];
	        this.refresh_minutes = source["refresh_minutes"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

}

export namespace main {
	
	export class Status {
	    count: number;
	    scanning: boolean;
	    last_refresh: string;
	    scan_milliseconds: number;
	    warnings: string[];
	    hotkey_error: string;
	    visible: boolean;
	
	    static createFrom(source: any = {}) {
	        return new Status(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.count = source["count"];
	        this.scanning = source["scanning"];
	        this.last_refresh = source["last_refresh"];
	        this.scan_milliseconds = source["scan_milliseconds"];
	        this.warnings = source["warnings"];
	        this.hotkey_error = source["hotkey_error"];
	        this.visible = source["visible"];
	    }
	}

}

export namespace search {
	
	export class Result {
	    id: string;
	    name: string;
	    path: string;
	    exec_path: string;
	    arguments: string;
	    working_directory: string;
	    icon_path: string;
	    icon_url: string;
	    description: string;
	    source: string;
	    keywords: string[];
	    score: number;
	
	    static createFrom(source: any = {}) {
	        return new Result(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.path = source["path"];
	        this.exec_path = source["exec_path"];
	        this.arguments = source["arguments"];
	        this.working_directory = source["working_directory"];
	        this.icon_path = source["icon_path"];
	        this.icon_url = source["icon_url"];
	        this.description = source["description"];
	        this.source = source["source"];
	        this.keywords = source["keywords"];
	        this.score = source["score"];
	    }
	}

}

