(() => {
  let lightbox = null;
  let lightboxImage = null;
  let lightboxStatus = null;
  let activeImages = [];
  let activeIndex = 0;

  document.addEventListener('click', (event) => {
    const button = event.target.closest('[data-carousel-prev], [data-carousel-next]');
    if (button) {
      const carousel = button.closest('[data-image-carousel]');
      const viewport = carousel && carousel.querySelector('.image-carousel__viewport');
      if (!viewport) {
        return;
      }

      const direction = button.hasAttribute('data-carousel-prev') ? -1 : 1;
      viewport.scrollBy({
        left: direction * viewport.clientWidth * 0.9,
        behavior: 'smooth',
      });
      return;
    }

    const imageLink = event.target.closest('.image-carousel__item a');
    if (imageLink) {
      event.preventDefault();
      const carousel = imageLink.closest('[data-image-carousel]');
      const links = Array.from(carousel.querySelectorAll('.image-carousel__item a'));
      activeImages = links.map((link) => ({
        href: link.href,
        alt: link.querySelector('img')?.alt || '',
      }));
      activeIndex = Math.max(0, links.indexOf(imageLink));
      openLightbox();
    }
  });

  document.addEventListener('keydown', (event) => {
    if (!lightbox || !lightbox.classList.contains('image-lightbox--active')) {
      return;
    }

    if (event.key === 'Escape') {
      closeLightbox();
    } else if (event.key === 'ArrowLeft') {
      showImage(activeIndex - 1);
    } else if (event.key === 'ArrowRight') {
      showImage(activeIndex + 1);
    }
  });

  function ensureLightbox() {
    if (lightbox) {
      return;
    }

    lightbox = document.createElement('div');
    lightbox.className = 'image-lightbox';
    lightbox.setAttribute('role', 'dialog');
    lightbox.setAttribute('aria-modal', 'true');
    lightbox.setAttribute('aria-label', 'Image viewer');
    lightbox.innerHTML = `
      <button type="button" class="image-lightbox__close" data-lightbox-close aria-label="Close image viewer">Close</button>
      <button type="button" class="image-lightbox__nav image-lightbox__nav--prev" data-lightbox-prev aria-label="Previous image">Prev</button>
      <figure class="image-lightbox__figure">
        <img class="image-lightbox__image" alt="">
        <figcaption class="image-lightbox__status"></figcaption>
      </figure>
      <button type="button" class="image-lightbox__nav image-lightbox__nav--next" data-lightbox-next aria-label="Next image">Next</button>
    `;
    document.body.appendChild(lightbox);

    lightboxImage = lightbox.querySelector('.image-lightbox__image');
    lightboxStatus = lightbox.querySelector('.image-lightbox__status');

    lightbox.addEventListener('click', (event) => {
      if (
        event.target === lightbox ||
        event.target.closest('[data-lightbox-close]')
      ) {
        closeLightbox();
        return;
      }
      if (event.target.closest('[data-lightbox-prev]')) {
        showImage(activeIndex - 1);
        return;
      }
      if (event.target.closest('[data-lightbox-next]')) {
        showImage(activeIndex + 1);
      }
    });
  }

  function openLightbox() {
    ensureLightbox();
    showImage(activeIndex);
    lightbox.classList.add('image-lightbox--active');
    document.body.classList.add('image-lightbox-open');
    lightbox.querySelector('[data-lightbox-close]').focus();
  }

  function closeLightbox() {
    lightbox.classList.remove('image-lightbox--active');
    document.body.classList.remove('image-lightbox-open');
  }

  function showImage(index) {
    if (!activeImages.length) {
      return;
    }

    activeIndex = (index + activeImages.length) % activeImages.length;
    const image = activeImages[activeIndex];
    lightboxImage.src = image.href;
    lightboxImage.alt = image.alt;
    lightboxStatus.textContent = `${activeIndex + 1} / ${activeImages.length}`;

    const hasMultiple = activeImages.length > 1;
    lightbox.querySelectorAll('[data-lightbox-prev], [data-lightbox-next]').forEach((button) => {
      button.hidden = !hasMultiple;
    });
  }
})();
