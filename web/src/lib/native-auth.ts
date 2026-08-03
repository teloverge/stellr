import { invoke, isTauri } from '@tauri-apps/api/core'

interface DeviceFlowPrompt {
  user_code: string
  verification_uri: string
  expires_in_seconds: number
  interval_seconds: number
}

export type DeviceFlowStatus =
  | { state: 'idle' }
  | ({ state: 'pending' } & DeviceFlowPrompt)
  | ({ state: 'slow_down' } & DeviceFlowPrompt)
  | { state: 'authorized' }
  | { state: 'denied' }
  | { state: 'expired' }
  | { state: 'cancelled' }
  | { state: 'failed'; message: string }

export function hasNativeAuth(): boolean {
  return isTauri()
}

export function beginDeviceAuthorization(): Promise<DeviceFlowStatus> {
  return invoke<DeviceFlowStatus>('begin_device_authorization')
}

export function deviceAuthorizationStatus(): Promise<DeviceFlowStatus> {
  return invoke<DeviceFlowStatus>('device_authorization_status')
}

export async function cancelDeviceAuthorization(): Promise<DeviceFlowStatus> {
  await invoke<void>('cancel_device_authorization')
  return deviceAuthorizationStatus()
}
