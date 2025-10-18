// Simple image lightbox
document.addEventListener('DOMContentLoaded', function() {
  // Create lightbox container
  const lightbox = document.createElement('div');
  lightbox.className = 'lightbox';
  lightbox.innerHTML = `
    <div class="lightbox__overlay"></div>
    <div class="lightbox__content">
      <img class="lightbox__image" src="" alt="">
      <button class="lightbox__close" aria-label="Close">&times;</button>
    </div>
  `;
  document.body.appendChild(lightbox);

  const overlay = lightbox.querySelector('.lightbox__overlay');
  const image = lightbox.querySelector('.lightbox__image');
  const closeBtn = lightbox.querySelector('.lightbox__close');

  // Function to open lightbox
  function openLightbox(imgSrc) {
    image.src = imgSrc;
    lightbox.classList.add('lightbox--active');
    document.body.style.overflow = 'hidden';
  }

  // Function to close lightbox
  function closeLightbox() {
    lightbox.classList.remove('lightbox--active');
    document.body.style.overflow = '';
  }

  // Add click handlers to all images in post-media and post-card__media
  document.querySelectorAll('.post-media__item img, .post-card__media img').forEach(img => {
    img.style.cursor = 'pointer';
    img.addEventListener('click', function(e) {
      e.preventDefault();
      openLightbox(this.src);
    });
  });

  // Close on click
  closeBtn.addEventListener('click', closeLightbox);
  overlay.addEventListener('click', closeLightbox);

  // Close on ESC key
  document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape' && lightbox.classList.contains('lightbox--active')) {
      closeLightbox();
    }
  });
});
