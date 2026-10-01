import {
  buildCameraConstraints,
  CAMERA_RESOLUTIONS,
  captureCameraFrame,
  mimeTypeForFormat
} from './cameraCapture'

describe('cameraCapture', () => {
  test('quality maps to the Electron tool resolutions', () => {
    expect(CAMERA_RESOLUTIONS).toEqual({
      low: { width: 640, height: 480 },
      medium: { width: 1280, height: 720 },
      high: { width: 1920, height: 1080 }
    })
  })

  test('constraints pin a device unless it is the default camera', () => {
    expect(buildCameraConstraints(undefined)).toEqual({
      video: { width: { ideal: 1280 }, height: { ideal: 720 } },
      audio: false
    })
    expect(buildCameraConstraints('default', 'low')).toEqual({
      video: { width: { ideal: 640 }, height: { ideal: 480 } },
      audio: false
    })
    expect(buildCameraConstraints('cam-1', 'high')).toEqual({
      video: { width: { ideal: 1920 }, height: { ideal: 1080 }, deviceId: { exact: 'cam-1' } },
      audio: false
    })
  })

  test('format → data URL mime type', () => {
    expect(mimeTypeForFormat('png')).toBe('image/png')
    expect(mimeTypeForFormat('jpg')).toBe('image/jpeg')
    expect(mimeTypeForFormat(undefined)).toBe('image/jpeg')
  })

  test('fails like the Electron tool without getUserMedia', async () => {
    await expect(captureCameraFrame({ requestId: 'r' })).rejects.toThrow(
      'getUserMedia API is not available in this browser'
    )
  })
})
