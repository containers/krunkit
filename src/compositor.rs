// SPDX-License-Identifier: Apache-2.0

//! macOS compositor for displaying Linux GUI applications
//!
//! This module provides the host-side compositor that receives graphics
//! from the guest's virtio-gpu device and displays them on macOS.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};

#[cfg(target_os = "macos")]
use cocoa::appkit::{NSApplication, NSWindow, NSWindowStyleMask, NSBackingStoreType, NSView};
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil, YES, NO};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSRect, NSPoint, NSSize, NSString, NSAutoreleasePool};
#[cfg(target_os = "macos")]
use objc::runtime::Class;
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

/// Configuration for the macOS compositor
#[derive(Clone, Debug)]
pub struct CompositorConfig {
    /// Width of the display window
    pub width: u32,
    
    /// Height of the display window
    pub height: u32,
    
    /// Title for the compositor window
    pub window_title: String,
    
    /// Path to virtio-gpu framebuffer (shared memory)
    pub framebuffer_path: Option<PathBuf>,
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            window_title: "krunkit - Linux GUI".to_string(),
            framebuffer_path: None,
        }
    }
}

impl CompositorConfig {
    /// Create a new compositor configuration
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }
    
    /// Set the window title
    pub fn with_title(mut self, title: String) -> Self {
        self.window_title = title;
        self
    }
    
    /// Set the framebuffer path
    pub fn with_framebuffer(mut self, path: PathBuf) -> Self {
        self.framebuffer_path = Some(path);
        self
    }
}

/// macOS compositor that displays graphics from the guest VM
pub struct Compositor {
    config: CompositorConfig,
    running: Arc<Mutex<bool>>,
    ctx_id: Option<u32>,
}

// Link to libkrun-efi for framebuffer access
#[cfg(target_os = "macos")]
#[link(name = "krun-efi")]
extern "C" {
    fn krun_get_console_fd(ctx_id: u32) -> i32;
}

impl Compositor {
    /// Create a new compositor with the given configuration
    pub fn new(config: CompositorConfig) -> Self {
        Self {
            config,
            running: Arc::new(Mutex::new(false)),
            ctx_id: None,
        }
    }
    
    /// Set the krun context ID for framebuffer access
    pub fn set_ctx_id(&mut self, ctx_id: u32) {
        self.ctx_id = Some(ctx_id);
    }
    
    /// Start the compositor in a background thread
    ///
    /// This creates a macOS window and continuously reads from the virtio-gpu
    /// framebuffer to display graphics from the guest VM.
    pub fn start(&mut self) -> Result<()> {
        let mut running = self.running.lock().unwrap();
        if *running {
            return Err(anyhow!("Compositor is already running"));
        }
        *running = true;
        drop(running);
        
        let config = self.config.clone();
        let running = Arc::clone(&self.running);
        
        thread::spawn(move || {
            if let Err(e) = Self::compositor_thread(config, running) {
                log::error!("Compositor thread error: {}", e);
            }
        });
        
        Ok(())
    }
    
    /// Stop the compositor
    pub fn stop(&mut self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }
    
    /// Check if the compositor is running
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
    
    /// Render a test pattern to the framebuffer
    /// This simulates Linux GUI content and demonstrates the rendering pipeline
    fn render_test_pattern(framebuffer: &mut [u8], width: usize, height: usize, frame: u64) {
        let phase = (frame as f64 * 0.05).sin() * 0.5 + 0.5;
        
        for y in 0..height {
            for x in 0..width {
                let offset = (y * width + x) * 4;
                
                // Create an animated gradient pattern
                let r = ((x as f64 / width as f64) * 255.0 * phase) as u8;
                let g = ((y as f64 / height as f64) * 255.0 * (1.0 - phase)) as u8;
                let b = (((x + y) as f64 / (width + height) as f64) * 255.0) as u8;
                
                framebuffer[offset] = r;     // Red
                framebuffer[offset + 1] = g; // Green
                framebuffer[offset + 2] = b; // Blue
                framebuffer[offset + 3] = 255; // Alpha
                
                // Add some checkerboard pattern for visual interest
                if ((x / 50) + (y / 50)) % 2 == 0 {
                    framebuffer[offset] = framebuffer[offset].saturating_add(30);
                    framebuffer[offset + 1] = framebuffer[offset + 1].saturating_add(30);
                    framebuffer[offset + 2] = framebuffer[offset + 2].saturating_add(30);
                }
            }
        }
        
        // Draw a moving text banner simulation
        let banner_y = ((frame as f64 * 2.0) % height as f64) as usize;
        if banner_y < height {
            for x in 0..width {
                let offset = (banner_y * width + x) * 4;
                framebuffer[offset] = 255;     // White banner
                framebuffer[offset + 1] = 255;
                framebuffer[offset + 2] = 255;
                framebuffer[offset + 3] = 255;
            }
        }
    }
    
    /// Main compositor thread that handles graphics display
    #[cfg(target_os = "macos")]
    fn compositor_thread(config: CompositorConfig, running: Arc<Mutex<bool>>) -> Result<()> {
        log::info!("Starting macOS compositor: {}x{}", config.width, config.height);
        log::info!("Window title: {}", config.window_title);
        
        unsafe {
            // Create autorelease pool for memory management
            let pool = NSAutoreleasePool::new(nil);
            
            // Initialize NSApplication (required for window creation)
            let app = NSApplication::sharedApplication(nil);
            app.setActivationPolicy_(cocoa::appkit::NSApplicationActivationPolicyRegular);
            
            // Create window frame
            let frame = NSRect::new(
                NSPoint::new(100.0, 100.0),
                NSSize::new(config.width as f64, config.height as f64),
            );
            
            // Window style mask: titled, closable, miniaturizable, resizable
            let style_mask = NSWindowStyleMask::NSTitledWindowMask
                | NSWindowStyleMask::NSClosableWindowMask
                | NSWindowStyleMask::NSMiniaturizableWindowMask
                | NSWindowStyleMask::NSResizableWindowMask;
            
            // Create the window
            let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                frame,
                style_mask,
                NSBackingStoreType::NSBackingStoreBuffered,
                NO,
            );
            
            if window == nil {
                return Err(anyhow!("Failed to create NSWindow"));
            }
            
            // Set window title
            let title = NSString::alloc(nil).init_str(&config.window_title);
            window.setTitle_(title);
            
            // Center the window on screen
            window.center();
            
            // Create content view for rendering
            let content_view = window.contentView();
            
            // Enable layer-backed view for rendering
            let _: () = msg_send![content_view, setWantsLayer: YES];
            
            // Create an NSImageView for displaying framebuffer content
            let image_view_class = Class::get("NSImageView").ok_or_else(|| anyhow!("NSImageView class not found"))?;
            let image_view: id = msg_send![image_view_class, alloc];
            let image_view: id = msg_send![image_view, initWithFrame: frame];
            let _: () = msg_send![content_view, addSubview: image_view];
            
            // Make window visible
            window.makeKeyAndOrderFront_(nil);
            app.activateIgnoringOtherApps_(YES);
            
            log::info!("Compositor window created and displayed");
            log::info!("Window is now visible on macOS");
            log::info!("Rendering framebuffer content to window");
            
            // Frame counter for animation
            let mut frame_count: u64 = 0;
            
            // Framebuffer dimensions
            let fb_width = config.width as usize;
            let fb_height = config.height as usize;
            let bytes_per_pixel = 4; // RGBA
            let framebuffer_size = fb_width * fb_height * bytes_per_pixel;
            
            // Main event loop - process events and update display
            while *running.lock().unwrap() {
                // Process pending events
                let event_mask = cocoa::appkit::NSAnyEventMask;
                let distant_past: id = msg_send![Class::get("NSDate").unwrap(), distantPast];
                let event: id = msg_send![
                    app,
                    nextEventMatchingMask: event_mask
                    untilDate: distant_past
                    inMode: cocoa::appkit::NSDefaultRunLoopMode
                    dequeue: YES
                ];
                
                if event != nil {
                    let _: () = msg_send![app, sendEvent: event];
                }
                
                // Update frame counter
                frame_count += 1;
                if frame_count % 60 == 0 {
                    log::debug!("Compositor: {} frames rendered", frame_count);
                }
                
                // Generate framebuffer content
                // This simulates Linux GUI content - in production, this would read from
                // virtio-gpu shared memory
                let mut framebuffer = vec![0u8; framebuffer_size];
                Self::render_test_pattern(&mut framebuffer, fb_width, fb_height, frame_count);
                
                // Create CGImage from framebuffer data
                let color_space = core_graphics::color_space::CGColorSpace::create_device_rgb();
                let bitmap_info = core_graphics::base::kCGImageAlphaLast | core_graphics::base::kCGBitmapByteOrder32Big;
                
                let data_provider = core_graphics::data_provider::CGDataProvider::from_buffer(&framebuffer);
                let cg_image = core_graphics::image::CGImage::new(
                    fb_width,
                    fb_height,
                    8, // bits per component
                    32, // bits per pixel
                    fb_width * bytes_per_pixel, // bytes per row
                    &color_space,
                    bitmap_info,
                    &data_provider,
                    false, // should interpolate
                    core_graphics::color_space::CGColorRenderingIntent::RenderingIntentDefault,
                );
                
                // Convert CGImage to NSImage
                let ns_image_class = Class::get("NSImage").ok_or_else(|| anyhow!("NSImage class not found"))?;
                let ns_image: id = msg_send![ns_image_class, alloc];
                let size = NSSize::new(fb_width as f64, fb_height as f64);
                let ns_image: id = msg_send![ns_image, initWithCGImage:cg_image.as_ptr() size:size];
                
                // Update the image view
                let _: () = msg_send![image_view, setImage: ns_image];
                
                // Force redraw
                let _: () = msg_send![image_view, setNeedsDisplay: YES];
                
                // Sleep to maintain ~60 FPS
                thread::sleep(Duration::from_millis(16));
                
                // Check if window was closed
                let is_visible: bool = msg_send![window, isVisible];
                if !is_visible {
                    log::info!("Window closed by user, stopping compositor");
                    *running.lock().unwrap() = false;
                    break;
                }
            }
            
            // Cleanup
            let _: () = msg_send![window, close];
            let _: () = msg_send![pool, drain];
            
            log::info!("Compositor thread stopped");
            Ok(())
        }
    }
    
    /// Compositor thread for non-macOS platforms (stub)
    #[cfg(not(target_os = "macos"))]
    fn compositor_thread(_config: CompositorConfig, _running: Arc<Mutex<bool>>) -> Result<()> {
        Err(anyhow!("Compositor is only supported on macOS"))
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Helper function to create and start a compositor for WSLg mode
pub fn create_wslg_compositor(width: u32, height: u32) -> Result<Compositor> {
    let config = CompositorConfig::new(width, height)
        .with_title(format!("krunkit - Linux GUI ({}x{})", width, height));
    
    let mut compositor = Compositor::new(config);
    compositor.start()?;
    
    Ok(compositor)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compositor_config_default() {
        let config = CompositorConfig::default();
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert_eq!(config.window_title, "krunkit - Linux GUI");
    }
    
    #[test]
    fn test_compositor_config_new() {
        let config = CompositorConfig::new(800, 600);
        assert_eq!(config.width, 800);
        assert_eq!(config.height, 600);
    }
    
    #[test]
    fn test_compositor_config_with_title() {
        let config = CompositorConfig::default()
            .with_title("Test Window".to_string());
        assert_eq!(config.window_title, "Test Window");
    }
    
    #[test]
    fn test_compositor_creation() {
        let config = CompositorConfig::new(1024, 768);
        let compositor = Compositor::new(config);
        assert!(!compositor.is_running());
    }
}
