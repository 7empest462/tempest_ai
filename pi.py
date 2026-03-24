{
  "tool": "extract_and_write",
  "arguments": {
    "path": "pi.py",
    "content": "def chudnovsky_pi(n):\n    pi_digits = [14, 0, 5, 2, 8]\n    q = 640320**3 // 256\n    k = 6\n\n    for i in range(1, n+1):\n        pi_digits.append(q * (pi_digits[2] - pi_digits[1] * pi_digits[0]) // (pi_digits[3] + i * pi_digits[4]))\n        q *= -(6 * (i + 1) * (i + 1)) // ((i + 1) * (i + 1) + i * i)\n        k += 5\n\n    return pi_digits[0], pi_digits[1]\n\n# Calculate the first 10 digits of π using Chudnovsky's algorithm\npi_digits = chudnovsky_pi(10)\n\n# Print the result\nprint(\"π ≈\", \".\" + \'\'.join(str(digit) for digit in pi_digits))\n"
  }
}