/**
 * Camera frame capture for the cameraCapture tool under Tauri.
 *
 * Under Electron the tool ran in the renderer (src/preload/tools/handlers/system/CameraCaptureTool.ts)
 * and captured with getUserMedia before sending the image to main. Under Tauri the tool runs in
 * Rust, which asks the main window for a frame with the `camera:capture-request` event; this
 * module grabs it with the same `CameraStreamManager` logic and the bridge answers with
 * `camera_capture_response`.
 */

export type CameraQuality = 'low' | 'medium' | 'high'
export type CameraFormat = 'jpg' | 'png'

export interface CameraFrameRequest {
  requestId?: string
  deviceId?: string
  quality?: CameraQuality
  format?: CameraFormat
}

export interface CapturedFrame {
  base64Data: string
  width: number
  height: number
  deviceId: string
  deviceName: string
}

/** Quality → ideal resolution. */
export const CAMERA_RESOLUTIONS: Record<CameraQuality, { width: number; height: number }> = {
  low: { width: 640, height: 480 },
  medium: { width: 1280, height: 720 },
  high: { width: 1920, height: 1080 }
}

/** `getUserMedia` constraints for a device and quality (`'default'` / none → any camera). */
export function buildCameraConstraints(
  deviceId: string | undefined,
  quality: CameraQuality = 'medium'
): MediaStreamConstraints {
  const resolution = CAMERA_RESOLUTIONS[quality] ?? CAMERA_RESOLUTIONS.medium
  return {
    video: {
      width: { ideal: resolution.width },
      height: { ideal: resolution.height },
      ...(deviceId && deviceId !== 'default' ? { deviceId: { exact: deviceId } } : {})
    },
    audio: false
  }
}

/** `format === 'png' ? 'image/png' : 'image/jpeg'`. */
export const mimeTypeForFormat = (format: CameraFormat | undefined): string =>
  format === 'png' ? 'image/png' : 'image/jpeg'

class CameraStreamManager {
  private currentStream: MediaStream | null = null
  private videoElement: HTMLVideoElement | null = null
  private canvasElement: HTMLCanvasElement | null = null

  async initializeCamera(
    deviceId?: string,
    quality: CameraQuality = 'medium'
  ): Promise<{ stream: MediaStream; deviceInfo: MediaDeviceInfo | null }> {
    try {
      const stream = await navigator.mediaDevices.getUserMedia(
        buildCameraConstraints(deviceId, quality)
      )
      this.currentStream = stream

      const devices = await navigator.mediaDevices.enumerateDevices()
      const videoDevices = devices.filter((device) => device.kind === 'videoinput')

      let deviceInfo: MediaDeviceInfo | null = null
      if (deviceId && deviceId !== 'default') {
        deviceInfo = videoDevices.find((device) => device.deviceId === deviceId) || null
      } else {
        const videoTrack = stream.getVideoTracks()[0]
        if (videoTrack) {
          const settings = videoTrack.getSettings()
          deviceInfo =
            videoDevices.find((device) => device.deviceId === settings.deviceId) ||
            videoDevices[0] ||
            null
        }
      }
      return { stream, deviceInfo }
    } catch (error) {
      throw new Error(
        `Failed to access camera: ${error instanceof Error ? error.message : String(error)}`
      )
    }
  }

  async captureImage(
    format: CameraFormat = 'jpg',
    quality: number = 0.9
  ): Promise<{ base64Data: string; width: number; height: number }> {
    if (!this.currentStream) {
      throw new Error('Camera stream not initialized')
    }
    if (!this.videoElement) {
      this.videoElement = document.createElement('video')
      this.videoElement.style.display = 'none'
      // WebKit only plays inline, muted video without a user gesture.
      this.videoElement.muted = true
      this.videoElement.playsInline = true
      document.body.appendChild(this.videoElement)
    }
    if (!this.canvasElement) {
      this.canvasElement = document.createElement('canvas')
      this.canvasElement.style.display = 'none'
      document.body.appendChild(this.canvasElement)
    }

    return new Promise((resolve, reject) => {
      const video = this.videoElement!
      const canvas = this.canvasElement!
      const timer = setTimeout(() => reject(new Error('Camera capture timeout')), 10000)

      video.onloadedmetadata = () => {
        try {
          const width = video.videoWidth
          const height = video.videoHeight
          canvas.width = width
          canvas.height = height
          const ctx = canvas.getContext('2d')
          if (!ctx) {
            reject(new Error('Failed to get canvas 2D context'))
            return
          }
          ctx.drawImage(video, 0, 0, width, height)
          resolve({
            base64Data: canvas.toDataURL(mimeTypeForFormat(format), quality),
            width,
            height
          })
        } catch (error) {
          reject(error)
        } finally {
          clearTimeout(timer)
        }
      }
      video.onerror = (error) => {
        clearTimeout(timer)
        reject(new Error(`Video loading error: ${error}`))
      }

      video.srcObject = this.currentStream
      video.play().catch(() => {
        // metadata still loads; the frame is drawn from the first decoded image
      })
    })
  }

  cleanup(): void {
    if (this.currentStream) {
      this.currentStream.getTracks().forEach((track) => track.stop())
      this.currentStream = null
    }
    if (this.videoElement) {
      document.body.removeChild(this.videoElement)
      this.videoElement = null
    }
    if (this.canvasElement) {
      document.body.removeChild(this.canvasElement)
      this.canvasElement = null
    }
  }
}

/** Open the camera, wait for it to settle, grab one frame, release the camera. */
export async function captureCameraFrame(request: CameraFrameRequest): Promise<CapturedFrame> {
  const quality = request.quality || 'medium'
  const format = request.format || 'jpg'
  if (
    typeof navigator === 'undefined' ||
    !navigator.mediaDevices ||
    !navigator.mediaDevices.getUserMedia
  ) {
    throw new Error('getUserMedia API is not available in this browser')
  }
  const manager = new CameraStreamManager()
  try {
    const { deviceInfo } = await manager.initializeCamera(request.deviceId, quality)
    // Wait a moment for the camera to stabilize (as the Electron tool did).
    await new Promise((resolve) => setTimeout(resolve, 1000))
    const frame = await manager.captureImage(format, 0.9)
    return {
      ...frame,
      deviceId: deviceInfo?.deviceId || 'default',
      deviceName: deviceInfo?.label || 'Unknown Camera'
    }
  } finally {
    manager.cleanup()
  }
}
