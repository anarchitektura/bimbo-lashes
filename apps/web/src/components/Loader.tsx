export default function Loader() {
  return (
    <div class="flex justify-center py-8">
      <div
        class="w-8 h-8 rounded-full border-3 border-t-transparent animate-spin"
        style={{
          "border-color": "var(--btn)",
          "border-top-color": "transparent",
          "border-width": "3px",
        }}
      />
    </div>
  );
}
