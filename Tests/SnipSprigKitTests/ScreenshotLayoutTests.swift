import Foundation
import CoreGraphics
import Testing
@testable import SnipSprigKit

@Suite struct ScreenshotLayoutTests {
    @Test func oversizedRetinaImageFitsWithoutDistortion() {
        let screen = CGRect(x: 0, y: 0, width: 1200, height: 800)
        let size = ScreenshotLayout.fittedSize(CGSize(width: 3000, height: 1500), on: screen)
        #expect(size.width <= screen.width - 40)
        #expect(size.height <= screen.height - 40)
        #expect(abs(size.width / size.height - 2) < 0.001)
    }

    @Test func smallImagesAreNotUpscaled() {
        let size = CGSize(width: 100, height: 50)
        #expect(ScreenshotLayout.fittedSize(size, on: CGRect(x: 0, y: 0, width: 1200, height: 800)) == size)
    }

    @Test(arguments: [
        CGRect(x: 0, y: 0, width: 1440, height: 875),
        CGRect(x: -1920, y: 0, width: 1920, height: 1055),
        CGRect(x: 0, y: -1080, width: 1920, height: 1055)
    ])
    func placementStaysVisibleAtEveryCorner(screen: CGRect) {
        let points = [CGPoint(x: screen.minX, y: screen.minY), CGPoint(x: screen.maxX, y: screen.maxY),
                      CGPoint(x: screen.maxX, y: screen.minY), CGPoint(x: screen.minX, y: screen.maxY)]
        for point in points {
            let frame = ScreenshotLayout.frame(size: CGSize(width: 700, height: 400), near: point, on: screen)
            #expect(screen.contains(frame))
        }
    }

    @Test func veryTallImageFitsVertically() {
        let screen = CGRect(x: 0, y: 0, width: 1200, height: 800)
        let size = ScreenshotLayout.fittedSize(CGSize(width: 200, height: 12000), on: screen)
        #expect(abs(size.height - 760) < 0.001)
        #expect(size.width > 0)
    }
}
