/**
 * Car App Showcase — Core Scripts
 * Minimal theme-toggling logic for the simplified showcase layout.
 */

document.addEventListener('DOMContentLoaded', () => {
    let isDarkMode = true;
    const body = document.body;
    const themeToggleBtn = document.getElementById('theme-toggle');

    function setTheme(dark) {
        isDarkMode = dark;
        
        if (isDarkMode) {
            body.classList.remove('light-theme');
            body.classList.add('dark-theme');
            
            // Header Toggle Icon
            themeToggleBtn.innerHTML = '<i class="fa-solid fa-moon"></i>';
            themeToggleBtn.title = 'Switch to Light Mode';
        } else {
            body.classList.remove('dark-theme');
            body.classList.add('light-theme');
            
            // Header Toggle Icon
            themeToggleBtn.innerHTML = '<i class="fa-solid fa-sun"></i>';
            themeToggleBtn.title = 'Switch to Dark Mode';
        }
    }

    // Handle theme toggle click
    if (themeToggleBtn) {
        themeToggleBtn.addEventListener('click', () => setTheme(!isDarkMode));
    }
});
