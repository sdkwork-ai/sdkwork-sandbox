import type { SandboxInstanceId } from './sandbox-instance-id';
import type { SandboxInstanceName } from './sandbox-instance-name';
import type { SandboxInstanceState } from './sandbox-instance-state';
import type { SandboxVersion } from './sandbox-version';

export interface SandboxInstance {
  sandboxInstanceId: SandboxInstanceId;
  sandboxInstanceName?: SandboxInstanceName;
  sandboxInstanceOwnerId?: string;
  sandboxInstanceState: SandboxInstanceState;
  sandboxInstanceCreatedAt?: string;
  sandboxInstanceExpiresAt?: string | null;
  sandboxVersion: SandboxVersion;
}
