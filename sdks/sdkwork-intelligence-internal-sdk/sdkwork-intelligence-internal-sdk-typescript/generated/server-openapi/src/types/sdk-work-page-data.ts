import type { PageInfo } from './page-info';
import type { SandboxInstance } from './sandbox-instance';

export interface SdkWorkPageData {
  items: SandboxInstance[];
  pageInfo: PageInfo;
}
