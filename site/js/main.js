// ===== Internationalization =====
const translations = {
    zh: {
        skipToContent: '跳转到主要内容',
        navModels: '机型',
        navFeatures: '特性',
        navArchitecture: '架构',
        navQuickStart: '快速开始',
        heroBadge: 'Rust 编写 · 低级仿真',
        heroDesc: '以 Rust 编写的文曲星电子词典模拟器，采用低级仿真(LLE)技术运行真实固件。<br>支持 NC1020、PC1000、CC800、NC2000、NC3000 五款经典机型。',
        btnDownload: '下载',
        btnSource: '源码',
        statModels: '款机型',
        statCPU: 'CPU',
        statLCD: 'LCD',
        statTests: '测试',
        scrollHint: '向下探索',
        aboutTitle: '何为文曲星？',
        aboutDesc: '文曲星是中国金远见公司生产的系列电子词典，自1990年代末起风靡全国。它不仅是一台词典，更是无数80、90后的青春记忆——<br><br>在那个智能手机尚未普及的年代，文曲星承载了我们的课堂笔记、课间游戏、和对编程的最初探索。',
        flowDevice: '文曲星硬件',
        flowDump: '固件 Dump',
        flowEmu: 'WQXEmu',
        flowPlay: '重温经典',
        modelsTitle: '支持机型',
        modelsSubtitle: '五款经典文曲星机型，完整硬件仿真',
        statusWorking: '正常运行',
        nc1020Desc: '经典机型，启动到主菜单，键盘和 NOR 正常工作',
        pc1000Desc: '启动到主菜单，键盘和 NOR 正常工作',
        cc800Desc: '启动到主菜单，键盘和 NOR 正常工作',
        nc2000Desc: '启动到时钟界面，待机/唤醒功能正常',
        nc3000Desc: '启动到时钟界面，待机/唤醒功能正常',
        featuresTitle: '核心特性',
        featuresSubtitle: '精确的硬件仿真，完整的功能实现',
        featureCPUTitle: '6502/W65C02 CPU',
        featureCPUDesc: '周期精确的指令执行，完整支持 BCD 模式和所有寻址模式',
        featureLCDTitle: 'LCD 显示',
        featureLCDDesc: '160×80 像素，4 灰度级别，支持残影和格栅效果',
        featureKeyboardTitle: '键盘输入',
        featureKeyboardDesc: '完整的 QWERTY 键盘矩阵仿真，支持多机型键位布局',
        featureTimerTitle: '定时器系统',
        featureTimerDesc: '多个定时器源，中断生成，RTC 实时时钟',
        featureAudioTitle: '音频系统',
        featureAudioDesc: 'SPDS104A DSP 仿真，支持音调生成和音频输出',
        featureRetroArchTitle: 'RetroArch 核心',
        featureRetroArchDesc: 'libretro 集成，支持 RetroArch 前端的所有功能',
        archTitle: '技术架构',
        archSubtitle: '模块化设计，清晰的层次结构',
        archFrontend: '独立桌面前端',
        archLibretro: 'RetroArch 核心',
        archCore: '平台无关仿真引擎',
        archMemory: '内存总线',
        archInput: '键盘输入',
        archAudio: '音频',
        archTimer: '定时器',
        archMachines: '机型实现',
        qsTitle: '快速开始',
        qsSubtitle: '几步即可运行文曲星模拟器',
        qsStep1Title: '获取固件',
        qsStep1Desc: '从文曲星硬件 dump 固件文件，或从社区获取',
        qsStep2Title: '下载 WQXEmu',
        qsStep2Desc: '从 GitHub Releases 下载对应平台的二进制文件',
        qsStep3Title: '运行模拟器',
        qsFirmwareTitle: '固件要求',
        qsTableModel: '机型',
        qsTableRequired: '必需固件',
        qsTableOptional: '可选固件',
        buildTitle: '从源码构建',
        buildSubtitle: '需要 Rust 工具链',
        buildStandaloneTitle: '独立模式',
        buildLibretroTitle: 'Libretro 核心',
        buildLibretroNote: '编译后的核心文件可加载到 RetroArch 中使用',
        linksTitle: '相关链接',
        linkReleases: '发布版本',
        linkCI: 'CI 构建',
        footerLicense: '采用 GNU 通用公共许可证 v3.0 或更高版本授权'
    },
    en: {
        skipToContent: 'Skip to main content',
        navModels: 'Models',
        navFeatures: 'Features',
        navArchitecture: 'Architecture',
        navQuickStart: 'Quick Start',
        heroBadge: 'Written in Rust · Low-Level Emulation',
        heroDesc: 'A Wenquxing electronic dictionary emulator written in Rust, using Low-Level Emulation (LLE) to run real firmware.<br>Supports NC1020, PC1000, CC800, NC2000, and NC3000 models.',
        btnDownload: 'Download',
        btnSource: 'Source',
        statModels: 'Models',
        statCPU: 'CPU',
        statLCD: 'LCD',
        statTests: 'Tests',
        scrollHint: 'Scroll to explore',
        aboutTitle: 'What is Wenquxing?',
        aboutDesc: 'Wenquxing is a series of electronic dictionaries produced by China\'s Gold Vision Company, which became popular nationwide since the late 1990s. It\'s not just a dictionary, but a cherished memory for countless people born in the 80s and 90s—<br><br>In an era before smartphones became ubiquitous, Wenquxing carried our class notes, recess games, and first explorations into programming.',
        flowDevice: 'Wenquxing HW',
        flowDump: 'Firmware Dump',
        flowEmu: 'WQXEmu',
        flowPlay: 'Relive Classics',
        modelsTitle: 'Supported Models',
        modelsSubtitle: 'Five classic Wenquxing models with full hardware emulation',
        statusWorking: 'Working',
        nc1020Desc: 'Classic model, boots to menu, keyboard and NOR work',
        pc1000Desc: 'Boots to menu, keyboard and NOR work',
        cc800Desc: 'Boots to menu, keyboard and NOR work',
        nc2000Desc: 'Boots to clock screen, standby/wake works',
        nc3000Desc: 'Boots to clock screen, standby/wake works',
        featuresTitle: 'Core Features',
        featuresSubtitle: 'Precise hardware emulation with complete functionality',
        featureCPUTitle: '6502/W65C02 CPU',
        featureCPUDesc: 'Cycle-accurate instruction execution with full BCD mode and addressing mode support',
        featureLCDTitle: 'LCD Display',
        featureLCDDesc: '160×80 pixels, 4 grayscale levels, with ghosting and grid effects',
        featureKeyboardTitle: 'Keyboard Input',
        featureKeyboardDesc: 'Complete QWERTY keyboard matrix emulation with multi-model key layouts',
        featureTimerTitle: 'Timer System',
        featureTimerDesc: 'Multiple timer sources, interrupt generation, RTC real-time clock',
        featureAudioTitle: 'Audio System',
        featureAudioDesc: 'SPDS104A DSP emulation with tone generation and audio output',
        featureRetroArchTitle: 'RetroArch Core',
        featureRetroArchDesc: 'libretro integration with full RetroArch frontend support',
        archTitle: 'Architecture',
        archSubtitle: 'Modular design with clear layer structure',
        archFrontend: 'Standalone Desktop',
        archLibretro: 'RetroArch Core',
        archCore: 'Platform-Independent Engine',
        archMemory: 'Memory Bus',
        archInput: 'Keyboard Input',
        archAudio: 'Audio',
        archTimer: 'Timer',
        archMachines: 'Machine Impls',
        qsTitle: 'Quick Start',
        qsSubtitle: 'Run the emulator in a few steps',
        qsStep1Title: 'Get Firmware',
        qsStep1Desc: 'Dump firmware from Wenquxing hardware, or obtain from community',
        qsStep2Title: 'Download WQXEmu',
        qsStep2Desc: 'Download the binary for your platform from GitHub Releases',
        qsStep3Title: 'Run Emulator',
        qsFirmwareTitle: 'Firmware Requirements',
        qsTableModel: 'Model',
        qsTableRequired: 'Required',
        qsTableOptional: 'Optional',
        buildTitle: 'Build from Source',
        buildSubtitle: 'Requires Rust toolchain',
        buildStandaloneTitle: 'Standalone Mode',
        buildLibretroTitle: 'Libretro Core',
        buildLibretroNote: 'The compiled core can be loaded into RetroArch',
        linksTitle: 'Related Links',
        linkReleases: 'Releases',
        linkCI: 'CI Builds',
        footerLicense: 'Licensed under the GNU General Public License v3.0 or later'
    }
};

let currentLang = 'zh';

function setLanguage(lang) {
    currentLang = lang;
    document.documentElement.lang = lang === 'zh' ? 'zh-CN' : 'en';

    document.querySelectorAll('[data-i18n]').forEach(el => {
        const key = el.getAttribute('data-i18n');
        if (translations[lang][key]) {
            el.innerHTML = translations[lang][key];
        }
    });
}

// ===== Ink Wash Canvas Animation =====
class InkParticle {
    constructor(canvas) {
        this.canvas = canvas;
        this.reset();
    }

    reset() {
        this.x = Math.random() * this.canvas.width;
        this.y = Math.random() * this.canvas.height;
        this.size = Math.random() * 3 + 1;
        this.speedX = (Math.random() - 0.5) * 0.5;
        this.speedY = (Math.random() - 0.5) * 0.5;
        this.opacity = Math.random() * 0.3 + 0.1;
        this.life = Math.random() * 200 + 100;
        this.maxLife = this.life;
        this.hue = Math.random() > 0.7 ? 0 : 210; // Red or blue ink
    }

    update() {
        this.x += this.speedX;
        this.y += this.speedY;
        this.life--;

        // Ink diffusion effect
        this.size += 0.02;
        this.opacity *= 0.998;

        if (this.life <= 0 || this.opacity < 0.01) {
            this.reset();
        }

        // Wrap around
        if (this.x < 0) this.x = this.canvas.width;
        if (this.x > this.canvas.width) this.x = 0;
        if (this.y < 0) this.y = this.canvas.height;
        if (this.y > this.canvas.height) this.y = 0;
    }

    draw(ctx) {
        ctx.beginPath();
        ctx.arc(this.x, this.y, this.size, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(${this.hue}, 60%, 30%, ${this.opacity})`;
        ctx.fill();
    }
}

function initInkCanvas() {
    const canvas = document.getElementById('inkCanvas');
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    const particles = [];
    const particleCount = 50;

    function resize() {
        canvas.width = canvas.offsetWidth;
        canvas.height = canvas.offsetHeight;
    }

    resize();
    window.addEventListener('resize', resize);

    // Create particles
    for (let i = 0; i < particleCount; i++) {
        particles.push(new InkParticle(canvas));
    }

    // Draw ink strokes
    function drawInkStroke(ctx, x, y, length, angle) {
        ctx.save();
        ctx.translate(x, y);
        ctx.rotate(angle);
        ctx.beginPath();
        ctx.moveTo(0, 0);

        // Organic stroke shape
        const cp1x = length * 0.3;
        const cp1y = Math.random() * 20 - 10;
        const cp2x = length * 0.7;
        const cp2y = Math.random() * 20 - 10;

        ctx.bezierCurveTo(cp1x, cp1y, cp2x, cp2y, length, 0);
        ctx.strokeStyle = 'rgba(30, 58, 95, 0.15)';
        ctx.lineWidth = Math.random() * 3 + 1;
        ctx.lineCap = 'round';
        ctx.stroke();
        ctx.restore();
    }

    // Draw some static ink strokes
    for (let i = 0; i < 5; i++) {
        drawInkStroke(
            ctx,
            Math.random() * canvas.width,
            Math.random() * canvas.height,
            Math.random() * 200 + 100,
            Math.random() * Math.PI
        );
    }

    function animate() {
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        // Draw subtle gradient background
        const gradient = ctx.createRadialGradient(
            canvas.width * 0.3, canvas.height * 0.3, 0,
            canvas.width * 0.3, canvas.height * 0.3, canvas.width * 0.7
        );
        gradient.addColorStop(0, 'rgba(30, 58, 95, 0.1)');
        gradient.addColorStop(1, 'rgba(15, 15, 26, 0)');
        ctx.fillStyle = gradient;
        ctx.fillRect(0, 0, canvas.width, canvas.height);

        // Update and draw particles
        particles.forEach(p => {
            p.update();
            p.draw(ctx);
        });

        // Draw connecting lines between nearby particles
        for (let i = 0; i < particles.length; i++) {
            for (let j = i + 1; j < particles.length; j++) {
                const dx = particles[i].x - particles[j].x;
                const dy = particles[i].y - particles[j].y;
                const dist = Math.sqrt(dx * dx + dy * dy);

                if (dist < 100) {
                    ctx.beginPath();
                    ctx.moveTo(particles[i].x, particles[i].y);
                    ctx.lineTo(particles[j].x, particles[j].y);
                    ctx.strokeStyle = `rgba(199, 75, 80, ${0.1 * (1 - dist / 100)})`;
                    ctx.lineWidth = 0.5;
                    ctx.stroke();
                }
            }
        }

        requestAnimationFrame(animate);
    }

    animate();
}

// ===== Device Showcase =====
function initDeviceShowcase() {
    const showcase = document.getElementById('deviceShowcase');
    if (!showcase) return;

    const images = showcase.querySelectorAll('.device-img');
    const dots = document.querySelectorAll('.device-dot');
    let currentIndex = 0;
    let interval;

    function showDevice(index) {
        images.forEach(img => img.classList.remove('active'));
        dots.forEach(dot => dot.classList.remove('active'));

        images[index].classList.add('active');
        dots[index].classList.add('active');
        currentIndex = index;
    }

    function nextDevice() {
        showDevice((currentIndex + 1) % images.length);
    }

    // Auto-rotate
    interval = setInterval(nextDevice, 3000);

    // Click handlers
    dots.forEach((dot, index) => {
        dot.addEventListener('click', () => {
            clearInterval(interval);
            showDevice(index);
            interval = setInterval(nextDevice, 3000);
        });
    });

    // Touch support
    let touchStartX = 0;
    showcase.addEventListener('touchstart', (e) => {
        touchStartX = e.touches[0].clientX;
        clearInterval(interval);
    });

    showcase.addEventListener('touchend', (e) => {
        const touchEndX = e.changedTouches[0].clientX;
        const diff = touchStartX - touchEndX;

        if (Math.abs(diff) > 50) {
            if (diff > 0) {
                showDevice((currentIndex + 1) % images.length);
            } else {
                showDevice((currentIndex - 1 + images.length) % images.length);
            }
        }

        interval = setInterval(nextDevice, 3000);
    });
}

// ===== Scroll Animations =====
function initScrollAnimations() {
    const observerOptions = {
        threshold: 0.1,
        rootMargin: '0px 0px -50px 0px'
    };

    const observer = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
            if (entry.isIntersecting) {
                entry.target.classList.add('visible');
                observer.unobserve(entry.target);
            }
        });
    }, observerOptions);

    // Add fade-in class to elements
    const animateElements = document.querySelectorAll(
        '.model-card, .feature-card, .qs-card, .build-card, .link-card, .about-grid, .arch-diagram'
    );

    animateElements.forEach((el, index) => {
        el.classList.add('fade-in');
        el.style.transitionDelay = `${index % 3 * 0.1}s`;
        observer.observe(el);
    });
}

// ===== Navigation =====
function initNavigation() {
    const nav = document.getElementById('nav');
    const mobileToggle = document.getElementById('mobileToggle');
    const navLinks = document.querySelector('.nav-links');

    // Scroll effect
    window.addEventListener('scroll', () => {
        if (window.scrollY > 50) {
            nav.classList.add('scrolled');
        } else {
            nav.classList.remove('scrolled');
        }
    });

    // Mobile toggle
    if (mobileToggle && navLinks) {
        mobileToggle.addEventListener('click', () => {
            navLinks.classList.toggle('active');
            mobileToggle.classList.toggle('active');
        });

        // Close on link click
        navLinks.querySelectorAll('a').forEach(link => {
            link.addEventListener('click', () => {
                navLinks.classList.remove('active');
                mobileToggle.classList.remove('active');
            });
        });
    }

    // Smooth scroll for anchor links
    document.querySelectorAll('a[href^="#"]').forEach(anchor => {
        anchor.addEventListener('click', function(e) {
            e.preventDefault();
            const target = document.querySelector(this.getAttribute('href'));
            if (target) {
                target.scrollIntoView({
                    behavior: 'smooth',
                    block: 'start'
                });
            }
        });
    });
}

// ===== Language Toggle =====
function initLanguageToggle() {
    const toggle = document.getElementById('langToggle');
    if (!toggle) return;

    // Check saved preference
    const savedLang = localStorage.getItem('wqxemu-lang');
    if (savedLang) {
        setLanguage(savedLang);
    }

    toggle.addEventListener('click', () => {
        const newLang = currentLang === 'zh' ? 'en' : 'zh';
        setLanguage(newLang);
        localStorage.setItem('wqxemu-lang', newLang);
    });
}

// ===== Parallax Effect =====
function initParallax() {
    const hero = document.querySelector('.hero');
    if (!hero) return;

    window.addEventListener('scroll', () => {
        const scrolled = window.pageYOffset;
        const rate = scrolled * -0.3;
        hero.style.backgroundPositionY = `${rate}px`;
    });
}

// ===== Initialize =====
document.addEventListener('DOMContentLoaded', () => {
    initInkCanvas();
    initDeviceShowcase();
    initScrollAnimations();
    initNavigation();
    initLanguageToggle();
    initParallax();
});
