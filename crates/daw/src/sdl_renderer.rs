use std::collections::HashMap;
use std::ffi::{CString, c_char, c_float, c_int};

use daw_core::render::{Color, FontSize, Renderer};
use sdl3_sys::rect::SDL_FRect;
use sdl3_sys::render::*;
use sdl3_sys::video::*;
use sdl3_ttf_sys::everything::*;

/// Number of font sizes: Small, Medium, Large, Title.
const FONT_COUNT: usize = 4;

/// Point sizes for each FontSize variant.
const FONT_SIZES: [c_float; FONT_COUNT] = [14.0, 18.0, 22.0, 28.0];

/// Maximum number of cached text objects before the cache is cleared.
const TEXT_CACHE_CAPACITY: usize = 512;

type TextCacheKey = (String, usize);

/// SDL3 renderer for the DAW.
pub struct SdlRenderer {
    window: *mut SDL_Window,
    renderer: *mut SDL_Renderer,
    text_engine: *mut TTF_TextEngine,
    fonts: [*mut TTF_Font; FONT_COUNT],
    width: f32,
    height: f32,
    text_cache: HashMap<TextCacheKey, *mut TTF_Text>,
}

impl SdlRenderer {
    /// Create a new SDL3 renderer with a window of the given dimensions.
    pub fn new(title: &str, width: u32, height: u32, font_path: &str) -> Result<Self, String> {
        let c_title = CString::new(title).map_err(|e| format!("invalid title: {e}"))?;
        let c_font_path = CString::new(font_path).map_err(|e| format!("invalid font path: {e}"))?;

        unsafe {
            if !sdl3_sys::init::SDL_Init(
                sdl3_sys::init::SDL_INIT_VIDEO | sdl3_sys::init::SDL_INIT_AUDIO,
            ) {
                return Err(sdl_error("SDL_Init"));
            }

            if !TTF_Init() {
                return Err("TTF_Init failed".to_string());
            }

            let window = SDL_CreateWindow(
                c_title.as_ptr(),
                width as c_int,
                height as c_int,
                SDL_WindowFlags(0),
            );
            if window.is_null() {
                return Err(sdl_error("SDL_CreateWindow"));
            }

            let renderer = SDL_CreateRenderer(window, std::ptr::null());
            if renderer.is_null() {
                SDL_DestroyWindow(window);
                return Err(sdl_error("SDL_CreateRenderer"));
            }

            // Enable alpha blending for transparent rendering.
            SDL_SetRenderDrawBlendMode(renderer, sdl3_sys::blendmode::SDL_BLENDMODE_BLEND);

            let text_engine = TTF_CreateRendererTextEngine(renderer);
            if text_engine.is_null() {
                SDL_DestroyRenderer(renderer);
                SDL_DestroyWindow(window);
                return Err("TTF_CreateRendererTextEngine failed".to_string());
            }

            let mut fonts = [std::ptr::null_mut::<TTF_Font>(); FONT_COUNT];
            for (i, &size) in FONT_SIZES.iter().enumerate() {
                let font = TTF_OpenFont(c_font_path.as_ptr(), size);
                if font.is_null() {
                    for f in &fonts[..i] {
                        TTF_CloseFont(*f);
                    }
                    TTF_DestroyRendererTextEngine(text_engine);
                    SDL_DestroyRenderer(renderer);
                    SDL_DestroyWindow(window);
                    return Err(format!("TTF_OpenFont failed at size {size}"));
                }
                fonts[i] = font;
            }

            Ok(Self {
                window,
                renderer,
                text_engine,
                fonts,
                width: width as f32,
                height: height as f32,
                text_cache: HashMap::new(),
            })
        }
    }

    /// Return the raw SDL window pointer (for SDL_StartTextInput / SDL_StopTextInput).
    pub fn window(&self) -> *mut SDL_Window {
        self.window
    }

    fn font_size_index(size: FontSize) -> usize {
        match size {
            FontSize::Small => 0,
            FontSize::Medium => 1,
            FontSize::Large => 2,
            FontSize::Title => 3,
        }
    }

    fn get_or_create_text(&mut self, text: &str, size: FontSize) -> *mut TTF_Text {
        let idx = Self::font_size_index(size);
        let key = (text.to_string(), idx);

        if let Some(&cached) = self.text_cache.get(&key) {
            return cached;
        }

        if self.text_cache.len() >= TEXT_CACHE_CAPACITY {
            self.clear_text_cache();
        }

        let c_text = match CString::new(text) {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        unsafe {
            let font = self.fonts[idx];
            let text_obj = TTF_CreateText(self.text_engine, font, c_text.as_ptr(), 0);
            if !text_obj.is_null() {
                self.text_cache.insert(key, text_obj);
            }
            text_obj
        }
    }

    fn clear_text_cache(&mut self) {
        for (_, text_obj) in self.text_cache.drain() {
            unsafe {
                TTF_DestroyText(text_obj);
            }
        }
    }

    fn draw_text_internal(&mut self, text: &str, x: f32, y: f32, size: FontSize, color: Color) {
        let text_obj = self.get_or_create_text(text, size);
        if text_obj.is_null() {
            return;
        }
        unsafe {
            TTF_SetTextColor(text_obj, color.r, color.g, color.b, color.a);
            TTF_DrawRendererText(text_obj, x, y);
        }
    }
}

impl Renderer for SdlRenderer {
    fn clear(&mut self, color: Color) {
        unsafe {
            SDL_SetRenderDrawColor(self.renderer, color.r, color.g, color.b, color.a);
            SDL_RenderClear(self.renderer);
        }
    }

    fn present(&mut self) {
        unsafe {
            SDL_RenderPresent(self.renderer);
        }
    }

    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let rect = SDL_FRect { x, y, w, h };
        unsafe {
            SDL_SetRenderDrawColor(self.renderer, color.r, color.g, color.b, color.a);
            SDL_RenderFillRect(self.renderer, &rect);
        }
    }

    fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let rect = SDL_FRect { x, y, w, h };
        unsafe {
            SDL_SetRenderDrawColor(self.renderer, color.r, color.g, color.b, color.a);
            SDL_RenderRect(self.renderer, &rect);
        }
    }

    fn draw_hline(&mut self, x1: f32, x2: f32, y: f32, color: Color) {
        unsafe {
            SDL_SetRenderDrawColor(self.renderer, color.r, color.g, color.b, color.a);
            SDL_RenderLine(self.renderer, x1, y, x2, y);
        }
    }

    fn draw_vline(&mut self, x: f32, y1: f32, y2: f32, color: Color) {
        unsafe {
            SDL_SetRenderDrawColor(self.renderer, color.r, color.g, color.b, color.a);
            SDL_RenderLine(self.renderer, x, y1, x, y2);
        }
    }

    fn draw_text(&mut self, text: &str, x: f32, y: f32, size: FontSize, color: Color) {
        self.draw_text_internal(text, x, y, size, color);
    }

    fn text_size(&mut self, text: &str, size: FontSize) -> (f32, f32) {
        let text_obj = self.get_or_create_text(text, size);
        if text_obj.is_null() {
            return (0.0, 0.0);
        }
        unsafe {
            let mut w: c_int = 0;
            let mut h: c_int = 0;
            TTF_GetTextSize(text_obj, &mut w, &mut h);
            (w as f32, h as f32)
        }
    }

    fn screen_width(&self) -> f32 {
        self.width
    }

    fn screen_height(&self) -> f32 {
        self.height
    }
}

impl Drop for SdlRenderer {
    fn drop(&mut self) {
        self.clear_text_cache();
        unsafe {
            for font in &self.fonts {
                if !font.is_null() {
                    TTF_CloseFont(*font);
                }
            }
            if !self.text_engine.is_null() {
                TTF_DestroyRendererTextEngine(self.text_engine);
            }
            TTF_Quit();
            if !self.renderer.is_null() {
                SDL_DestroyRenderer(self.renderer);
            }
            if !self.window.is_null() {
                SDL_DestroyWindow(self.window);
            }
            sdl3_sys::init::SDL_Quit();
        }
    }
}

fn sdl_error(context: &str) -> String {
    let err = sdl3_sys::error::SDL_GetError();
    if err.is_null() {
        return format!("{context}: unknown error");
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(err as *const c_char) };
    format!("{context}: {}", c_str.to_string_lossy())
}
