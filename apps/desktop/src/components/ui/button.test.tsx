import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Button } from "./button";

describe("Button", () => {
  it("renders its label", () => {
    render(<Button>Import</Button>);
    expect(screen.getByRole("button", { name: "Import" })).toBeInTheDocument();
  });

  it("applies the variant classes", () => {
    render(<Button variant="destructive">Delete</Button>);
    expect(screen.getByRole("button", { name: "Delete" })).toHaveClass("bg-destructive");
  });
});
