import * as fs from 'fs';
import * as path from 'path';

export class FixtureRoot {
    private readonly rootPath: string;

    constructor(dir: string) {
        const marker = path.join(dir, '.custodian-fixture-root');
        if (!fs.existsSync(marker)) {
            throw new Error("SECURITY: Test attempted to use a non-fixture path. Path: " + dir);
        }
        this.rootPath = dir;
    }

    getPath(): string {
        return this.rootPath;
    }
}
