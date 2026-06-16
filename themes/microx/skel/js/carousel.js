(() => {
  let lightbox = null;
  let lightboxImage = null;
  let lightboxStatus = null;
  let activeImages = [];
  let activeIndex = 0;

  document.addEventListener('DOMContentLoaded', () => {
    document.querySelectorAll('[data-image-carousel]').forEach((carousel) => {
      updateCarouselDots(carousel);

      const viewport = carousel.querySelector('.image-carousel__viewport');
      if (!viewport) {
        return;
      }

      let isQueued = false;
      viewport.addEventListener('scroll', () => {
        if (isQueued) {
          return;
        }

        isQueued = true;
        requestAnimationFrame(() => {
          updateCarouselDots(carousel);
          isQueued = false;
        });
      }, { passive: true });
    });
  });

  document.addEventListener('click', (event) => {
    const dot = event.target.closest('[data-carousel-dot]');
    if (dot) {
      const carousel = dot.closest('[data-image-carousel]');
      const viewport = carousel && carousel.querySelector('.image-carousel__viewport');
      const items = carousel ? getCarouselItems(carousel) : [];
      const index = Number(dot.getAttribute('data-carousel-index') || 0);
      const item = items[index];
      if (!viewport || !item) {
        return;
      }

      item.scrollIntoView({
        behavior: 'smooth',
        block: 'nearest',
        inline: 'start',
      });
      updateCarouselDots(carousel, index);
      return;
    }

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
      updateCarouselDots(carousel);
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

  function getCarouselItems(carousel) {
    return Array.from(carousel.querySelectorAll('.media-carousel__item, .image-carousel__item'));
  }

  function updateCarouselDots(carousel, forcedIndex) {
    const dots = Array.from(carousel.querySelectorAll('[data-carousel-dot]'));
    if (!dots.length) {
      return;
    }

    const viewport = carousel.querySelector('.image-carousel__viewport');
    const items = getCarouselItems(carousel);
    if (!viewport || !items.length) {
      return;
    }

    let activeDot = Number.isFinite(forcedIndex) ? forcedIndex : 0;
    if (!Number.isFinite(forcedIndex)) {
      const viewportRect = viewport.getBoundingClientRect();
      const viewportCenter = viewportRect.left + viewportRect.width / 2;
      let closestDistance = Infinity;

      items.forEach((item, index) => {
        const itemRect = item.getBoundingClientRect();
        const itemCenter = itemRect.left + itemRect.width / 2;
        const distance = Math.abs(itemCenter - viewportCenter);
        if (distance < closestDistance) {
          closestDistance = distance;
          activeDot = index;
        }
      });
    }

    dots.forEach((dot, index) => {
      const isActive = index === activeDot;
      dot.classList.toggle('is-active', isActive);
      if (isActive) {
        dot.setAttribute('aria-current', 'true');
      } else {
        dot.removeAttribute('aria-current');
      }
    });
  }

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
